//! **One answer to "can the body reach this cell"**: the reach graph and every
//! consumer that counts reached cells asked the same generated cases, red on
//! any difference (CLAUDE.md, "One question, one implementation").
//!
//! The consumers are the blockout battery's place reachability (`DW0837`,
//! [`crate::compiler::blockout::reached_from`]), its unallocated-crossing
//! flood (`DW0838`, [`crate::compiler::blockout::crossing_flood`]) and the
//! breadth-first walk its leak route is named over ([`World::body_route`]), the moves
//! its contact and portal crossings step out of a wall by (`DW0877`, `DW0986`,
//! [`crate::compiler::blockout::steps_out`]), its pacing legs (`DW0822`,
//! [`crate::compiler::blockout::leg_routed`]), the piece contract's
//! `contract-reachability` walk ([`crate::grammar::contract::reached_shut`]),
//! seeded from the entry's mouth, its edge proof per class
//! ([`crate::grammar::contract::class_proved`]: `walk`, `stair`, `drop` and
//! `climb`), and the grammar gates `traversable`
//! ([`crate::grammar::gates::traversal_passes`], with and without falls) and
//! `reachable-floor` ([`crate::grammar::gates::floor_walked`] and the count
//! [`crate::grammar::gates::reachability_from`] reports), and the per-piece
//! floor readers: the light probe's measured floor
//! ([`crate::admit::light::floor_measured`], from the ground entry and from a
//! door), the sculpt's walk from grade ([`crate::sculpt::entry_walked`]) and
//! the enclosure survey's reached floor (`DW0895`,
//! [`crate::compiler::view::showing::floor_reached`] and the count
//! [`crate::compiler::view::showing::survey`] reports), and the nav-side
//! readers of where a body can be: `DW0891`'s player arm
//! ([`crate::compiler::nav::World::reach_into_volumes`]), a timed hazard's
//! pre-commitment ground (`DW0388`,
//! [`crate::compiler::nav::hazard::pre_commit_ground`], over the course with
//! its last platform walled up), a trigger-set checkpoint's root (`DW0315`,
//! [`crate::compiler::nav::checkpoint::earliest_reaching_config`]), a
//! `reach` objective's arriving cells (`DW0881`,
//! [`crate::compiler::reach::cells_that_reach`]), and the shut-gate model's
//! party reach (`DW1011`, `DW1053`/`DW1054`: `nav::shutmodel::party_reach`)
//! with the order the shut-way eye is asked in (`nav::learned::route_tree`,
//! every cell an eye stands in).
//!
//! The cases are parkour courses: platforms three wide along `x` with gaps of
//! zero to four columns, rises of up to two courses either way, a low ceiling
//! that forbids a jump, a pit under the gaps, and declared jumps across some
//! gaps. The battery is asked over the world with its declared jumps; a piece
//! contract and a grammar gate have no declared-jump surface, so they are
//! asked over the same blocks without them and compared with the graph over
//! those blocks. A `climb` proof needs a climbable to hold on, so it is asked
//! over the same course with a ladder hung down the first platform's west face
//! on a stone column, and compared with the graph over that piece's blocks.
//!
//! The per-piece readers start from the piece's ground entry, which needs
//! floor on a vertical face of the box: they are asked over the same course
//! with its first platform widened to the box's `z` faces, and compared with the
//! graph over those blocks from the same entry.
//!
//! The test also reds when its cases are vacuous: when no case needs a jump of
//! the launch set, or no case needs a declared jump, for the graph to reach
//! more than the walk does, or no step-out closure or leg needs a jump or a
//! fall, or no grammar consumer's answer needs one, or no climb case reaches
//! a held cell, or no per-piece reader's floor needs a jump or a fall, or no
//! nav-side reader's answer does.

use super::super::{Premises, World};
use crate::grammar::block::BlockState;
use crate::grammar::geom::Box3;
use crate::grammar::model::VoxelModel;
use crate::schem::prefab::{ContractEdge, ContractSpace, ContractVolume, Region, SpatialContract};
use std::collections::{BTreeMap, BTreeSet};

/// One generated course: its solid cells, the cell a body starts in, and the
/// jumps it declares.
struct Case {
    solid: BTreeSet<[i32; 3]>,
    root: [i32; 3],
    jumps: Vec<([i32; 3], [i32; 3])>,
    /// The model's extent, `x` and `y` (`z` is always 5).
    size: [u32; 2],
}

/// A fixed linear congruential sequence: the cases are the same on every run
/// (ADR-0006) without a seeded RNG crate.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> i32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % n) as i32
    }
}

/// The platforms run along `x` from `x = 1`, over `z` 1..=3; each is a
/// course of solid with the body's cells over it. The model leaves one cell
/// of air on every side and four courses over the highest platform.
fn generate(seed: u64) -> Case {
    let mut r = Lcg(seed);
    let mut solid = BTreeSet::new();
    let mut jumps = Vec::new();
    let pit = r.next(3) == 0;
    let base = 6;
    let mut x = 1;
    let mut y = base;
    let mut top = y;
    let mut prev_end: Option<[i32; 3]> = None;
    let platforms = 3 + r.next(2);
    let mut low_ceiling_over: Option<(i32, i32, i32)> = None;
    for i in 0..platforms {
        let len = 3 + r.next(4);
        if i > 0 {
            let gap = r.next(5);
            let rise = r.next(5) - 2;
            x += gap;
            y = (y + rise).clamp(base - 2, base + 4);
            if let Some(from) = prev_end
                && gap > 0
                && r.next(2) == 0
            {
                jumps.push((from, [x, y + 1, 2]));
            }
        }
        for dx in 0..len {
            for z in 1..=3 {
                solid.insert([x + dx, y, z]);
            }
        }
        if i == 1 && r.next(3) == 0 {
            low_ceiling_over = Some((x, x + len - 1, y + 3));
        }
        top = top.max(y);
        prev_end = Some([x + len - 1, y + 1, 2]);
        x += len;
    }
    if let Some((x0, x1, cy)) = low_ceiling_over {
        for cx in x0..=x1 {
            for z in 1..=3 {
                solid.insert([cx, cy, z]);
            }
        }
    }
    if pit {
        for px in 1..x {
            for z in 1..=3 {
                solid.insert([px, 1, z]);
            }
        }
    }
    Case {
        solid,
        root: [1, base + 1, 2],
        jumps,
        size: [(x + 1) as u32, (top + 6) as u32],
    }
}

fn model_of(case: &Case) -> VoxelModel {
    let mut m = VoxelModel::new(Box3 {
        origin: [0, 0, 0],
        size: [case.size[0], case.size[1], 5],
    });
    let stone = BlockState::simple("minecraft:stone_bricks");
    for c in &case.solid {
        m.set(*c, &stone).expect("one block");
    }
    m
}

/// The course with a ladder hung down the first platform's west face, from the
/// platform's own course to the bottom of the model, on a stone column under
/// the platform's middle row: the cells a body holds in are the ladder's.
fn laddered(case: &Case) -> (VoxelModel, BTreeSet<[i32; 3]>) {
    let mut m = model_of(case);
    let stone = BlockState::simple("minecraft:stone_bricks");
    let ladder: BlockState = "minecraft:ladder[facing=west]".parse().expect("a ladder");
    let top = case.root[1] - 1;
    let mut rungs = BTreeSet::new();
    for y in 1..=top {
        m.set([1, y, 2], &stone).expect("one block");
        m.set([0, y, 2], &ladder).expect("one block");
        rungs.insert([0, y, 2]);
    }
    (m, rungs)
}

/// The course with its first platform widened to the box's `z` faces (`z = 0`
/// and `z = 4`), so a ground entry exists there and nowhere else: the rest of
/// the course is floor a body gets to only by crossing from it. The solid
/// cells and the model.
fn faced(case: &Case) -> (BTreeSet<[i32; 3]>, VoxelModel) {
    let y = case.root[1] - 1;
    let mut end = case.root[0];
    while case.solid.contains(&[end, y, 1]) {
        end += 1;
    }
    let mut solid = case.solid.clone();
    for x in case.root[0]..end {
        solid.insert([x, y, 0]);
        solid.insert([x, y, 4]);
    }
    let wide = Case {
        solid: solid.clone(),
        root: case.root,
        jumps: Vec::new(),
        size: case.size,
    };
    (solid, model_of(&wide))
}

/// The model as the renderer's parsed structure, for the enclosure survey.
fn structure_of(model: &VoxelModel) -> crate::compiler::view::nbt::Structure {
    let size = model.region().size.map(|n| n as i32);
    let mut palette: Vec<String> = Vec::new();
    let mut blocks = Vec::new();
    for pos in model.region().positions() {
        if let Some(b) = model.get(pos)
            && !b.is_air()
        {
            let name = b.to_string();
            let i = match palette.iter().position(|p| *p == name) {
                Some(i) => i,
                None => {
                    palette.push(name);
                    palette.len() - 1
                }
            };
            blocks.push((pos, i));
        }
    }
    crate::compiler::view::nbt::Structure {
        size,
        palette,
        blocks,
    }
}

fn region(from: [i32; 3], to: [i32; 3]) -> Region {
    Region { from, to }
}

fn edge(a: &str, b: &str, class: &str) -> ContractEdge {
    ContractEdge {
        a: a.to_string(),
        b: b.to_string(),
        class: class.to_string(),
        rise: Some(0),
        via: None,
        bar: None,
        way: None,
        extra: Default::default(),
    }
}

/// A contract whose confinement never binds: the way in is the start column
/// pair `x = 0..=1` (the via of the entry's edges to `exterior`, a walk and a
/// drop each way), and the entry space is everything past it. The walk is
/// seeded from that mouth, which is the platform cells over `x = 1`.
fn open_contract(case: &Case) -> SpatialContract {
    let [sx, sy] = case.size;
    let (xmax, ymax) = (sx as i32 - 1, sy as i32 - 1);
    let mut spaces = BTreeMap::new();
    spaces.insert(
        "course".to_string(),
        ContractSpace {
            part: None,
            envelope: "open".to_string(),
            boxes: vec![region([2, 0, 0], [xmax, ymax, 4])],
        },
    );
    let mouth = |e: ContractEdge| ContractEdge {
        via: Some(ContractVolume {
            region: "mouth".to_string(),
            boxes: vec![region([0, 0, 0], [1, ymax, 4])],
        }),
        ..e
    };
    SpatialContract {
        entry: "course".to_string(),
        spaces,
        no_body: BTreeMap::new(),
        edges: vec![
            mouth(edge("course", "exterior", "walk")),
            mouth(edge("course", "exterior", "drop")),
            mouth(edge("exterior", "course", "drop")),
        ],
        faces: Vec::new(),
        no_body_majority_ack: None,
    }
}

#[test]
fn every_consumer_reaches_what_the_reach_graph_reaches() {
    const CASES: u64 = 48;
    let mut launch_needed = 0usize;
    let mut declared_needed = 0usize;
    let mut contract_launch_needed = 0usize;
    let mut steps_needed = 0usize;
    let mut legs_needed = 0usize;
    let mut declared = 0usize;
    let mut cells = 0usize;
    let mut legs = 0usize;
    let mut grammar_pairs = 0usize;
    let mut grammar_needed = 0usize;
    let mut held_reached = 0usize;
    let mut entries = 0usize;
    let mut floor_needed = 0usize;
    let mut volume_needed = 0usize;
    let mut watch_needed = 0usize;
    let mut root_needed = 0usize;
    let mut arriving_needed = 0usize;
    let mut eye_needed = 0usize;
    for seed in 0..CASES {
        let case = generate(seed);
        declared += case.jumps.len();
        let bare = World::from_solid_cells(case.solid.clone());
        let mut p = Premises::geometry_only();
        p.declared_jumps = case.jumps.clone();
        let jumping = bare.with_premises(p);

        // DW0837, over the world with its declared jumps.
        let graph = jumping.body_region(&[case.root]);
        let battery = crate::compiler::blockout::reached_from(&jumping, &[case.root]);
        assert_eq!(
            battery, graph,
            "case {seed}: DW0837's reached cells differ from the reach graph's"
        );
        let without = bare.body_region(&[case.root]);
        let walk = bare.walk_region(&[case.root]);
        assert!(
            without.is_superset(&walk),
            "case {seed}: the graph holds the walk"
        );
        if graph.len() > without.len() {
            declared_needed += 1;
        }
        if without.len() > walk.len() {
            launch_needed += 1;
        }
        cells += graph.len();

        // DW0838's flood, with nothing deleted and no commons: every state.
        let states = jumping.body_reach(&[case.root]).cells();
        let flood = crate::compiler::blockout::crossing_flood(&jumping, &[case.root], &|_| true);
        assert_eq!(
            flood, states,
            "case {seed}: DW0838's flood differs from the reach graph's states"
        );
        // DW0838's route: the breadth-first walk `World::body_route` takes,
        // run to the end, holds what the flood holds.
        let route_walk =
            jumping.confined_closure(&[case.root], &super::super::Confinement::none(), |_, _| {
                false
            });
        assert_eq!(
            route_walk, flood,
            "case {seed}: the leak route's walk differs from the flood it names a route in"
        );

        // DW0877 and DW0986 step out of a cell: their one-move answers, taken
        // to a fixpoint, are the graph's states.
        let mut stepped: BTreeSet<[i32; 3]> = BTreeSet::from([case.root]);
        let mut frontier = vec![case.root];
        while let Some(c) = frontier.pop() {
            for n in crate::compiler::blockout::steps_out(&jumping, c) {
                if stepped.insert(n) {
                    frontier.push(n);
                }
            }
        }
        assert_eq!(
            stepped, states,
            "case {seed}: DW0877/DW0986's moves out of a cell differ from the reach graph's"
        );
        if stepped
            .iter()
            .any(|c| !walk.contains(c) && graph.contains(c))
        {
            steps_needed += 1;
        }

        // DW0822's legs: from the root to every cell over a solid block.
        for c in &case.solid {
            let z = [c[0], c[1] + 1, c[2]];
            legs += 1;
            let routed = crate::compiler::blockout::leg_routed(&jumping, case.root, z);
            assert_eq!(
                routed,
                graph.contains(&z),
                "case {seed}: DW0822 routes the leg to {z:?} differently from the reach graph"
            );
            if routed && !walk.contains(&z) {
                legs_needed += 1;
            }
        }

        // The nav-side readers of where a body can be, over the world with its
        // declared jumps. `DW0891`'s player arm: the cells it floods into
        // are the graph's states.
        let volume = jumping.reach_into_volumes(
            &[case.root],
            &crate::compiler::nav::Footprint::player(),
            false,
            None,
            &[],
        );
        assert_eq!(
            volume.reached, states,
            "case {seed}: DW0891's player arm reaches cells the reach graph does not, or misses some"
        );
        let mut cell_flood: BTreeSet<[i32; 3]> = BTreeSet::from([case.root]);
        let mut frontier = vec![case.root];
        let fp = crate::compiler::nav::Footprint::player();
        while let Some(c) = frontier.pop() {
            for n in jumping.moves_of(c, &fp) {
                if cell_flood.insert(n) {
                    frontier.push(n);
                }
            }
        }
        if volume.reached.len() > cell_flood.len() {
            volume_needed += 1;
        }
        // A timed hazard's pre-commitment ground (`DW0388`'s watch bay): the
        // graph's standing states over the course with the hazard's span — the
        // last platform's feet cells — walled up, built here from the blocks.
        let last_x = case.solid.iter().map(|c| c[0]).max().expect("a course");
        let span: BTreeSet<[i32; 3]> = case
            .solid
            .iter()
            .filter(|c| c[0] == last_x)
            .map(|c| [c[0], c[1] + 1, c[2]])
            .collect();
        let mut walled_solid = case.solid.clone();
        walled_solid.extend(span.iter().copied());
        let mut wp = Premises::geometry_only();
        wp.declared_jumps = case.jumps.clone();
        let walled = World::from_solid_cells(walled_solid).with_premises(wp);
        let watch = crate::compiler::nav::hazard::pre_commit_ground(&jumping, &span, case.root);
        let walled_graph = walled.body_region(&[case.root]);
        assert_eq!(
            watch, walled_graph,
            "case {seed}: the hazard's pre-commitment ground differs from the reach graph's"
        );
        if watch.len() > walled.walk_region(&[case.root]).len() {
            watch_needed += 1;
        }
        // `DW0315`'s trigger root and `DW0881`'s arriving cells, toward every
        // cell over a solid block.
        let configs = vec![(None, 0usize, vec![case.root])];
        let region = jumping.body_reach(&[case.root]).cells();
        let arriving = crate::compiler::reach::cells_that_reach(
            &jumping,
            &region,
            &BTreeSet::from([case.root]),
        );
        for c in &case.solid {
            let z = [c[0], c[1] + 1, c[2]];
            let rooted = crate::compiler::nav::checkpoint::earliest_reaching_config(
                &configs,
                &jumping,
                &BTreeSet::from([z]),
            );
            assert_eq!(
                rooted,
                graph.contains(&z).then_some(0),
                "case {seed}: DW0315 roots a trigger at {z:?} differently from the reach graph"
            );
            if rooted.is_some() && !walk.contains(&z) {
                root_needed += 1;
            }
            if !region.contains(&z) {
                continue;
            }
            let back = jumping.body_reach(&[z]).cells().contains(&case.root);
            assert_eq!(
                arriving.contains(&z),
                back,
                "case {seed}: DW0881 judges {z:?} arriving differently from the reach graph"
            );
            if back && !jumping.walk_region(&[z]).contains(&case.root) {
                arriving_needed += 1;
            }
        }

        // The shut-gate model's party reach and the shut-way eye's order: both
        // hold exactly the graph's states.
        let (_, party) = crate::compiler::nav::shutmodel::party_reach(&jumping, &[case.root]);
        assert_eq!(
            party.cells(),
            states,
            "case {seed}: the shut-gate model's party reach differs from the reach graph's"
        );
        let (_, order) = crate::compiler::nav::learned::route_tree(&party, &[case.root]);
        let eyes: BTreeSet<[i32; 3]> = order.iter().copied().collect();
        assert_eq!(
            eyes.len(),
            order.len(),
            "case {seed}: the eye's order names a cell twice"
        );
        assert_eq!(
            eyes, states,
            "case {seed}: the shut-way eye stands in cells the reach graph does not, or misses some"
        );
        if eyes.iter().any(|c| !walk.contains(c)) {
            eye_needed += 1;
        }

        // The contract, over the same blocks: a piece declares no jump.
        let model = model_of(&case);
        let (targets, reached) =
            crate::grammar::contract::reached_shut(&model, &open_contract(&case));
        let starts: Vec<[i32; 3]> = targets.iter().filter(|c| c[0] <= 1).copied().collect();
        assert!(!starts.is_empty(), "case {seed}: the mouth has floor");
        let graph = bare.body_region(&starts);
        assert!(
            graph.is_subset(&targets),
            "case {seed}: the graph stands a body where the contract counts no floor: {:?}",
            graph.difference(&targets).collect::<Vec<_>>()
        );
        assert_eq!(
            reached, graph,
            "case {seed}: contract-reachability's reached cells differ from the reach graph's"
        );
        if reached.len() > bare.walk_region(&starts).len() {
            contract_launch_needed += 1;
        }

        // The grammar's own reach: `reachable-floor`'s walk and count, then
        // `traversable` and the edge proof per class, from the root to every
        // cell over a solid block and back.
        let standable = crate::grammar::nav::standable_cells(&model);
        let roots = BTreeSet::from([case.root]);
        let from_root = bare.body_region(&[case.root]);
        assert!(
            from_root.is_subset(&standable),
            "case {seed}: the graph stands a body where the grammar counts no floor"
        );
        let floor = crate::grammar::gates::floor_walked(&model, &standable, &roots);
        assert_eq!(
            floor, from_root,
            "case {seed}: reachable-floor's walk differs from the reach graph's"
        );
        assert_eq!(
            crate::grammar::gates::reachability_from(&model, &standable, &roots).reachable,
            from_root.len(),
            "case {seed}: reachable-floor's count differs from the reach graph's"
        );
        let walked_here = bare.walk_region(&[case.root]);
        if from_root.len() > walked_here.len() {
            grammar_needed += 1;
        }

        // The per-piece floor readers, over the course widened to its faces,
        // from its ground entry; and the light probe from a door at the root.
        let (wide_solid, wide) = faced(&case);
        let wide_world = World::from_solid_cells(wide_solid);
        let wide_standable = crate::grammar::nav::standable_cells(&wide);
        let entry = crate::grammar::nav::ground_entry(&wide);
        entries += entry.len();
        let seeds: Vec<[i32; 3]> = entry.iter().copied().collect();
        let graph_floor: BTreeSet<[i32; 3]> = wide_world
            .body_reach(&seeds)
            .cells()
            .intersection(&wide_standable)
            .copied()
            .collect();
        let crate::schem::nav::EnteredFloor {
            entry: light_entry,
            reached: light,
            ..
        } = crate::admit::light::floor_measured(&wide, &BTreeSet::new());
        assert_eq!(light_entry, entry, "case {seed}: the probe's entry");
        assert_eq!(
            light, graph_floor,
            "case {seed}: the light probe's measured floor differs from the reach graph's"
        );
        let sculpted = crate::sculpt::entry_walked(
            &crate::grammar::nav::body_world(&wide),
            &wide_standable,
            &entry,
        );
        assert_eq!(
            sculpted, graph_floor,
            "case {seed}: the sculpt's walk from grade differs from the reach graph's"
        );
        let st = structure_of(&wide);
        let st_cells = crate::compiler::view::showing::Cells::of(&st);
        let surveyed = crate::compiler::view::showing::floor_reached(&st, &st_cells).reached;
        assert_eq!(
            surveyed, graph_floor,
            "case {seed}: the enclosure survey's reached floor differs from the reach graph's"
        );
        assert_eq!(
            crate::compiler::view::showing::survey(&st).reached,
            graph_floor.len(),
            "case {seed}: DW0895's reached count differs from the reach graph's"
        );
        let walked_floor: BTreeSet<[i32; 3]> = wide_world
            .walk_region(&seeds)
            .intersection(&wide_standable)
            .copied()
            .collect();
        if !entry.is_empty() && graph_floor.len() > walked_floor.len() {
            floor_needed += 1;
        }
        let doored = crate::admit::light::floor_measured(&model, &roots).reached;
        assert_eq!(
            doored, from_root,
            "case {seed}: the light probe's floor from a door differs from the reach graph's"
        );
        let (ladder_model, rungs) = laddered(&case);
        let ladder_world = crate::grammar::contract::body_world(&ladder_model);
        let ladder_standable = crate::grammar::nav::standable_cells(&ladder_model);
        let ladder_cells: BTreeSet<[i32; 3]> = ladder_standable.union(&rungs).copied().collect();
        let ladder_reach = |from: [i32; 3]| {
            let states = ladder_world.body_reach(&[from]).cells();
            assert!(
                states.is_subset(&ladder_cells),
                "case {seed}: a body rests outside the climb's cells: {:?}",
                states.difference(&ladder_cells).collect::<Vec<_>>()
            );
            states
        };
        let ladder_root = ladder_reach(case.root);
        if !ladder_root.is_disjoint(&rungs) {
            held_reached += 1;
        }
        for c in &case.solid {
            let z = [c[0], c[1] + 1, c[2]];
            if !standable.contains(&z) {
                continue;
            }
            grammar_pairs += 1;
            let there = BTreeSet::from([z]);
            let forward = from_root.contains(&z);
            let back = bare.body_region(&[z]).contains(&case.root);
            if (forward && !walked_here.contains(&z))
                || (back && !bare.walk_region(&[z]).contains(&case.root))
            {
                grammar_needed += 1;
            }
            let ways = [roots.clone(), there.clone()];
            assert_eq!(
                crate::grammar::gates::traversal_passes(&model, &ways, false),
                forward && back,
                "case {seed}: traversable (no falls) judges the root and {z:?} differently \
                 from the reach graph"
            );
            assert_eq!(
                crate::grammar::gates::traversal_passes(&model, &ways, true),
                forward || back,
                "case {seed}: traversable (with falls) judges the root and {z:?} differently \
                 from the reach graph"
            );
            for class in ["walk", "stair"] {
                assert_eq!(
                    crate::grammar::contract::class_proved(
                        class, &model, &standable, &standable, &roots, &there
                    ),
                    forward && back,
                    "case {seed}: the `{class}` edge proof judges the root and {z:?} \
                     differently from the reach graph"
                );
            }
            assert_eq!(
                crate::grammar::contract::class_proved(
                    "drop", &model, &standable, &standable, &roots, &there
                ),
                forward && !back,
                "case {seed}: the `drop` edge proof judges the root and {z:?} differently \
                 from the reach graph"
            );
            if ladder_standable.contains(&z) {
                let climbed = ladder_root.contains(&z) && ladder_reach(z).contains(&case.root);
                assert_eq!(
                    crate::grammar::contract::class_proved(
                        "climb",
                        &ladder_model,
                        &ladder_standable,
                        &rungs,
                        &roots,
                        &there
                    ),
                    climbed,
                    "case {seed}: the `climb` edge proof judges the root and {z:?} \
                     differently from the reach graph"
                );
            }
        }
    }
    eprintln!(
        "reach agreement: {CASES} case(s), {cells} reached cell(s), {declared} declared jump(s), \
         {legs} leg(s); {launch_needed} case(s) need a launch-set jump, {declared_needed} a \
         declared jump, {contract_launch_needed} of the contract's a launch-set jump, \
         {steps_needed} of the step-out a jump or fall, {legs_needed} leg(s) a jump or fall; \
         {grammar_pairs} grammar pair(s), {grammar_needed} grammar answer(s) a jump or fall, \
         {held_reached} climb case(s) reaching a held cell; {entries} ground-entry cell(s), \
         {floor_needed} per-piece floor(s) a jump or fall; {volume_needed} DW0891 flood(s) \
         past the cell relation, {watch_needed} watch bay(s) past the walk, {root_needed} \
         trigger root(s) and {arriving_needed} arriving cell(s) a jump or fall; {eye_needed} \
         shut-way eye order(s) past the walk"
    );
    assert!(
        launch_needed > 0 && contract_launch_needed > 0,
        "no case needs a jump of the launch set: the agreement is vacuous"
    );
    assert!(
        declared_needed > 0,
        "no case needs a declared jump: the agreement is vacuous"
    );
    assert!(
        steps_needed > 0 && legs_needed > 0,
        "no step-out or leg needs more than the walk: the agreement is vacuous"
    );
    assert!(
        grammar_needed > 0 && held_reached > 0,
        "no grammar answer needs more than the walk, or no climb holds: the agreement is vacuous"
    );
    assert!(
        entries > 0 && floor_needed > 0,
        "no ground entry, or no per-piece floor needs more than the walk: the agreement is vacuous"
    );
    assert!(
        eye_needed > 0,
        "no shut-way eye order needs more than the walk: the agreement is vacuous"
    );
    assert!(
        volume_needed > 0 && watch_needed > 0 && root_needed > 0 && arriving_needed > 0,
        "no DW0891 flood, watch bay, trigger root or arriving cell needs more than the walk: \
         the agreement is vacuous"
    );
}

/// **Water at a box edge**: one piece's water spills into the air round it,
/// and a body that swam that spill would leave the piece and come back in
/// past a wall the piece closes. Every per-piece reader asks the same
/// confinement ([`World::of_piece`]'s box, kept by
/// [`World::body_flood`]): the light probe's floor
/// ([`crate::admit::light::floor_measured`]) and the fixtures' walk
/// ([`crate::grammar::nav::reaches`]) agree that the far side is not
/// reached, and the unconfined closure shows the spill was a way round, so
/// the agreement is not vacuous.
#[test]
fn water_at_a_box_edge_leaves_no_way_round_the_piece() {
    // x 0..9, y 0..6, z 0..4: a floor at y = 0, a wall at x = 4 from floor to
    // roof across the whole depth, and a pool of water at y = 1 along the
    // z = 0 face on both sides of the wall — water a body swims, at the face,
    // and spills out of it.
    let mut m = VoxelModel::new(Box3 {
        origin: [0, 0, 0],
        size: [9, 6, 4],
    });
    let stone = BlockState::simple("minecraft:stone_bricks");
    let water: BlockState = "minecraft:water[level=0]".parse().expect("water");
    for x in 0..9 {
        for z in 0..4 {
            m.set([x, 0, z], &stone).expect("one block");
        }
    }
    // The wall, and the shell on every other side: the box's ends (x = 0,
    // x = 8) and its back (z = 3), so the only face open is the pool's and no
    // floor stands on a face to be walked in from.
    for y in 1..6 {
        for z in 0..4 {
            for x in [0, 4, 8] {
                m.set([x, y, z], &stone).expect("one block");
            }
        }
        for x in 0..9 {
            m.set([x, y, 3], &stone).expect("one block");
        }
    }
    // The pool is dammed off the floor by a one-course lip at z = 1, so the
    // floor at z = 2 stays dry and only the face row holds water.
    for x in 0..9 {
        m.set([x, 1, 1], &stone).expect("one block");
    }
    for x in [1, 2, 3, 5, 6, 7] {
        m.set([x, 1, 0], &water).expect("one block");
    }
    let near: BTreeSet<[i32; 3]> = BTreeSet::from([[1, 1, 2]]);
    let far: BTreeSet<[i32; 3]> = BTreeSet::from([[7, 1, 2]]);
    let standable = crate::grammar::nav::standable_cells(&m);
    assert!(standable.contains(&[1, 1, 2]) && standable.contains(&[7, 1, 2]));

    // The spill is a way round when nothing keeps a body in the box.
    let mut blocks = std::collections::BTreeMap::new();
    for pos in m.region().positions() {
        if let Some(b) = m.get(pos) {
            blocks.insert(pos, b.to_string());
        }
    }
    let unboxed = World::of_piece(blocks, [i32::MIN; 3], [i32::MAX; 3]);
    let round = unboxed.body_flood(&[[1, 1, 2]], &super::super::Confinement::none());
    let outside = round
        .iter()
        .filter(|c| (0..3).any(|a| c[a] < 0 || c[a] >= [9, 6, 4][a]))
        .count();
    eprintln!(
        "water at a box edge: {} state(s) unboxed, {outside} outside the box; far side reached \
         unboxed: {}",
        round.len(),
        round.contains(&[7, 1, 2])
    );
    assert!(
        round.contains(&[7, 1, 2]) && outside > 0,
        "the spill is no way round, so the case binds nothing"
    );

    let probed = crate::admit::light::floor_measured(&m, &near);
    assert_eq!(
        probed.entry, near,
        "the probe is walked in from the door alone"
    );
    let light = probed.reached;
    let walked = crate::grammar::nav::reaches(&m, &standable, &near, &far);
    assert!(
        !light.contains(&[7, 1, 2]),
        "the light probe's floor swims out of the box and back in"
    );
    assert!(
        !walked,
        "the fixtures' walk swims out of the box and back in"
    );
    assert_eq!(
        light,
        crate::grammar::nav::body_world(&m).floor_reached(&standable, &near),
        "the light probe's floor and the piece's own world disagree"
    );
}

/// A node table's every list, in cell order.
type Table = Vec<([i32; 3], Vec<(super::Quadrants, super::NodeEdges)>)>;

/// Everything a closure answers, read through its own API: the states, every
/// state's successors and predecessors in order, the flags and the counts —
/// and the node tables of the graph it was taken over.
#[derive(Debug, PartialEq)]
struct Answered {
    states: Vec<([i32; 3], super::RestKind)>,
    succ: Vec<Vec<super::Edge>>,
    preds: Vec<Vec<([i32; 3], super::Edge)>>,
    census: [usize; 4],
    edge_census: [usize; 6],
    leaves_world: Vec<[i32; 3]>,
    open_sea: Vec<[i32; 3]>,
    fatal: Vec<([i32; 3], super::Death)>,
    work: (u64, u64),
    unflown: Vec<bool>,
    nodes: Table,
    flown: Table,
}

/// What `w`'s closure from `roots` answers, grown by every launch of every
/// standing and holding state until none is left unflown (a copy taken to
/// grow, [`World::fly_cells`]), taken with new lists at chunk side `side`.
/// Flying to that fixpoint is what makes the node tables comparable across
/// sides: a graph's memo is shared by every test that asks for the world at
/// that side, and one that flew a node this reading had not would otherwise
/// show in the tables of one side and not the other.
fn answered(w: &World, roots: &[[i32; 3]], side: i32) -> Answered {
    super::chunks::at_side(side, || {
        let mut r = w.body_reach(roots);
        loop {
            let unflown: Vec<[i32; 3]> = r.states.keys().filter(|c| r.unflown(*c)).collect();
            if unflown.is_empty() {
                break;
            }
            w.fly_cells(&mut r, &unflown);
        }
        let memo = w.graph_memo(&super::ReachParams::shipped(w.unbounded_fall));
        let cells: Vec<[i32; 3]> = r.states.keys().collect();
        let strip = |l: Table| {
            l.into_iter()
                .map(|(c, v)| {
                    let mut v = v;
                    v.sort_by_key(|(q, _)| *q);
                    (c, v)
                })
                .collect::<Vec<_>>()
        };
        let succ: Vec<Vec<super::Edge>> = cells.iter().map(|c| r.succ(*c)).collect();
        let preds: Vec<Vec<([i32; 3], super::Edge)>> = cells.iter().map(|c| r.preds(*c)).collect();
        // The records read back from the lists are the edges counted at join
        // (ADR-0037 §2): a list that lost or doubled a record at every side
        // alike would otherwise answer the same at every side.
        let joined: usize =
            r.edge_census().iter().sum::<usize>() - r.leaves_world.len() - r.open_sea.len();
        assert_eq!(
            succ.iter().map(Vec::len).sum::<usize>(),
            joined,
            "the successor records read back are not the edges counted at join"
        );
        assert_eq!(
            preds.iter().map(Vec::len).sum::<usize>(),
            joined,
            "the predecessor records read back are not the edges counted at join"
        );
        Answered {
            states: r.states.iter().collect(),
            succ,
            preds,
            census: r.census(),
            edge_census: r.edge_census(),
            leaves_world: r.leaves_world.iter().copied().collect(),
            open_sea: r.open_sea.iter().copied().collect(),
            fatal: r.fatal.iter().map(|(c, d)| (*c, *d)).collect(),
            work: (r.simulated, r.credited),
            unflown: cells.iter().map(|c| r.unflown(*c)).collect(),
            nodes: strip(memo.nodes.entries()),
            flown: strip(memo.flown.entries()),
        }
    })
}

/// **No answer of the reach graph depends on its chunk side or its resident
/// budget** (ADR-0037 §4): every generated course, the Bell yard (a void's
/// edge), the jetty (the open sea) and the garden, asked of a closure whose
/// edge lists and node tables are kept in chunks of sixteen columns, of seven,
/// and of two under a budget of one byte — so every list is cut across chunks
/// and every chunk is written out and read back on every use. Red on any
/// difference in any answer: the states, every state's successors and
/// predecessors in order, the flags, the counts and both node tables. (The
/// sides are this test's own: a graph is kept per side, and the node tables of
/// one other tests share hold what those tests flew.) The graph is built at
/// four threads whatever `DELVEC_THREADS` says: a snapshot page is read back
/// only past a thread's recent pages, and at one thread none is, so the
/// binding below would be zero (spec-0126 §5.4).
#[test]
fn no_answer_depends_on_the_chunk_side_or_the_budget() {
    crate::par::with_threads(4, no_answer_depends_on_the_chunk_side_or_the_budget_at)
}

fn no_answer_depends_on_the_chunk_side_or_the_budget_at() {
    let worlds = invariance_worlds();
    let (mut states, mut edges, mut sea, mut crossing, mut holding) = (0, 0, 0, 0, 0);
    let mut read_back = 0u64;
    for (name, w, roots) in &worlds {
        let whole = answered(w, roots, 16);
        let odd = answered(w, roots, 7);
        let cut = super::budget::with_budget(1, || answered(w, roots, 2));
        // The snapshot cut to one resident page of each type, over a graph of
        // its own (a side no other case takes): every read past a thread's
        // recent pages goes back to the file.
        let loads = super::pages::traffic().2;
        let paged = super::budget::with_budget(1, || answered(w, roots, 5));
        read_back += super::pages::traffic().2 - loads;
        assert_eq!(
            whole, paged,
            "{name}: the graph over a snapshot of one resident page answers otherwise"
        );
        assert_eq!(
            whole, odd,
            "{name}: the graph in seven-column chunks answers otherwise"
        );
        assert_eq!(
            whole, cut,
            "{name}: the graph cut in two-column chunks answers otherwise"
        );
        states += whole.states.len();
        holding += whole.census[3];
        edges += whole.succ.iter().map(Vec::len).sum::<usize>();
        sea += whole.open_sea.len();
        crossing += whole
            .states
            .iter()
            .zip(&whole.succ)
            .flat_map(|((c, _), es)| es.iter().map(move |e| (*c, e.to)))
            .filter(|(a, b)| {
                (a[0].div_euclid(2), a[2].div_euclid(2)) != (b[0].div_euclid(2), b[2].div_euclid(2))
            })
            .count();
    }
    eprintln!(
        "chunk invariance: {} world(s), {states} state(s) ({holding} holding), {edges} edge(s) \
         ({crossing} crossing a two-column chunk face), {sea} open-sea state(s); {read_back} \
         snapshot page(s) read back under a budget of one",
        worlds.len()
    );
    assert!(
        !worlds.is_empty()
            && states > 0
            && holding > 0
            && edges > 0
            && crossing > 0
            && sea > 0
            && read_back > 0,
        "the cases bind nothing"
    );
}

/// A floor at y = 64 over x 0..=6, z 0..=2, a solid mass at x 3..=6 up to
/// y = 68 (stood on at y = 69), and a ladder hanging on its west face at
/// x = 2, z = 1, y 65..=68: the one way up is four holding states.
fn ladder_cliff() -> World {
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..=6 {
        for z in 0..=2 {
            cells.push(([x, 64, z], "minecraft:stone"));
            if x >= 3 {
                for y in 65..=68 {
                    cells.push(([x, y, z], "minecraft:stone"));
                }
            }
        }
    }
    for y in 65..=68 {
        cells.push(([2, y, 1], "minecraft:ladder[facing=west]"));
    }
    crate::compiler::nav::testkit::blocks_world(&cells)
}

/// A reef under an ocean (sea level 62, floor 54): a pillar rising out of the
/// sea at x 5..=7, z 0..=2 (stood on at y 67), and beside it a walled basin,
/// x 8..=15, z 0..=7, its walls up to y 64 and its inside water to y 63 — a
/// second water body the sea does not touch. From the pillar a body steps
/// onto the wall and jumps into either water: the plain sea in two
/// components, with the surface, the floor layer and the cells beside every
/// wall as their boundary.
fn reef() -> World {
    let mut b = super::tests::Build::default();
    b.fill([5, 55, 0], [7, 66, 2]);
    for y in 55..=64 {
        for x in 8..=15 {
            for z in 0..=7 {
                if x == 8 || x == 15 || z == 0 || z == 7 {
                    b.fill([x, y, z], [x, y, z]);
                }
            }
        }
    }
    for y in 55..=63 {
        for x in 9..=14 {
            for z in 1..=6 {
                b.water.insert([x, y, z]);
            }
        }
    }
    let built = vec![
        ("prefab/pillar".to_string(), ([5, 55, 0], [7, 66, 2])),
        ("prefab/basin".to_string(), ([8, 54, 0], [15, 64, 7])),
    ];
    b.world(
        super::Ambient::Ocean(crate::compiler::nav::Sea {
            level: 62,
            floor_top: 54,
        }),
        built,
    )
}

/// **The ground in the sea's band is no water** (ADR-0037 §2, the plain sea
/// measured): under an ocean, a cell the assembly wrote a block into — the
/// terrain's ground, outside every placed piece's box — is no rest state and
/// is never entered, however deep in the sea's band it lies; the sea beside
/// it is. A headland of stone through the band (y 55..=66 over x 0..=9, z
/// 0..=9, no `built` box) with a body on top: the closure from the top holds
/// every sea state it reaches and no cell of the stone, and no stone cell is
/// a rest state. Refuses a run that reaches no sea. Perturbation: ambient
/// water judged by the pieces' boxes alone → red (the stone's band cells are
/// submerged states and the closure fills the headland).
#[test]
fn the_ground_in_the_seas_band_is_no_water() {
    let mut b = super::tests::Build::default();
    b.fill([0, 55, 0], [9, 66, 9]);
    let w = b.world(
        super::Ambient::Ocean(crate::compiler::nav::Sea {
            level: 62,
            floor_top: 54,
        }),
        Vec::new(),
    );
    let stone = |c: &[i32; 3]| {
        (0..=9).contains(&c[0]) && (55..=66).contains(&c[1]) && (0..=9).contains(&c[2])
    };
    for x in 0..=9 {
        for z in 0..=9 {
            for y in 55..=62 {
                assert_eq!(
                    w.rest_kind([x, y, z]),
                    None,
                    "stone at {:?} rests a body",
                    [x, y, z]
                );
            }
        }
    }
    assert_eq!(w.rest_kind([-1, 62, 5]), Some(super::RestKind::Afloat));
    assert_eq!(w.rest_kind([-1, 60, 5]), Some(super::RestKind::Submerged));
    let r = w.body_reach(&[[5, 67, 5]]);
    let sea = r.states.keys().filter(|c| w.ambient_water(*c)).count();
    let inside = r.states.keys().filter(stone).count();
    eprintln!(
        "headland: {} state(s), {sea} in the sea, {inside} in the stone",
        r.states.len()
    );
    assert!(sea > 0, "the case binds nothing: the sea is not reached");
    assert_eq!(inside, 0, "the closure entered the stone");
}

/// The worlds the chunk invariance and the closure's exactness are asked of:
/// every generated course, the Bell yard (a void's edge), the jetty (the open
/// sea), the garden, the ladder cliff (holding states) and the reef (the
/// plain sea in two components).
fn invariance_worlds() -> Vec<(String, World, Vec<[i32; 3]>)> {
    let mut worlds: Vec<(String, World, Vec<[i32; 3]>)> = Vec::new();
    for seed in 0..12 {
        let case = generate(seed);
        let mut p = Premises::geometry_only();
        p.declared_jumps = case.jumps.clone();
        let w = World::from_solid_cells(case.solid.clone()).with_premises(p);
        worlds.push((format!("course {seed}"), w, vec![case.root]));
    }
    worlds.push((
        "the Bell yard".into(),
        super::tests::bell_yard(),
        vec![[5, 1, 5]],
    ));
    worlds.push(("the jetty".into(), super::tests::jetty(), vec![[2, 65, 2]]));
    worlds.push(("the garden".into(), super::tests::garden(), vec![[1, 1, 1]]));
    worlds.push(("the ladder cliff".into(), ladder_cliff(), vec![[0, 65, 1]]));
    worlds.push(("the reef".into(), reef(), vec![[6, 67, 1]]));
    worlds
}

/// **A closure's rounds leave no state with a direction to fly**: the round
/// asks only the states the last close reached ([`World::round`]), on the
/// ground that a state asked once asks for nothing after — its envelope only
/// shrinks as the closure grows. Held here directly: every invariance world's
/// closure, asked of every standing and holding state against what it flew,
/// has none left — and a state asked in a round before the last was asked
/// (the cases bind nothing otherwise). Perturbation: a round that asks a
/// state late, or an envelope that grows, leaves a state with a direction.
#[test]
fn the_rounds_leave_no_state_with_a_direction_to_fly() {
    let worlds = invariance_worlds();
    let (mut states, mut early, mut worlds_n) = (0usize, 0usize, 0usize);
    for (name, w, roots) in &worlds {
        let r = w.body_reach(roots);
        let memo = w.graph_memo(&super::ReachParams::shipped(w.unbounded_fall));
        let nodes: Vec<[i32; 3]> = r
            .states
            .iter()
            .filter(|(_, k)| matches!(k, super::RestKind::Standing | super::RestKind::Holding))
            .map(|(c, _)| c)
            .collect();
        let left = w.left_to_fly(&memo, &r, &nodes, &|c| r.flown_mask(c));
        assert!(
            left.is_empty(),
            "{name}: {} state(s) the rounds left with a direction to fly, the first {:?}",
            left.len(),
            left.first()
        );
        // A state flown toward fewer than every direction was asked in a
        // round before the closure was whole: had it been asked again later
        // it could only have been for nothing.
        early += nodes
            .iter()
            .filter(|c| {
                let q = r.flown_mask(**c);
                q != 0 && q != super::ALL_QUADRANTS
            })
            .count();
        states += nodes.len();
        worlds_n += 1;
    }
    eprintln!(
        "rounds: {worlds_n} world(s), {states} standing or holding state(s), none with a \
         direction left; {early} flown toward some directions and not others"
    );
    assert!(
        states > 0 && early > 0,
        "the cases bind nothing: no state was flown toward some directions and not others"
    );
}

/// **The summarised sea is the closure expanded cell by cell** (ADR-0037 §2):
/// every invariance world asked of the closure that floods the plain sea by
/// component and computes its dives on read, and of the closure that expands
/// every water cell as a node of its own (the test knob `unsummarised`, at a
/// side of its own) — every state, every state's successors and predecessors
/// in order, the flags, the counts and the work equal; the node table of the
/// summarised graph holds no plain-sea cell, and with those cells taken out
/// of the other's the two tables are equal. Refuses a run with no plain-sea
/// state, no world of two components, and no boundary (a water state that is
/// not plain).
#[test]
fn the_summarised_sea_is_the_closure_expanded_cell_by_cell() {
    let worlds = invariance_worlds();
    let (mut plain, mut boundary, mut two) = (0usize, 0usize, 0usize);
    for (name, w, roots) in &worlds {
        let whole = answered(w, roots, 16);
        let expanded = super::unsummarised(|| answered(w, roots, 13));
        let r = super::chunks::at_side(16, || w.body_reach(roots));
        let plain_here: BTreeSet<[i32; 3]> =
            r.states.keys().filter(|c| r.states.plain(c)).collect();
        for (a, b, what) in [
            (
                format!("{:?}", whole.states),
                format!("{:?}", expanded.states),
                "states",
            ),
            (
                format!("{:?}", whole.succ),
                format!("{:?}", expanded.succ),
                "succ",
            ),
            (
                format!("{:?}", whole.preds),
                format!("{:?}", expanded.preds),
                "preds",
            ),
            (
                format!("{:?}", whole.census),
                format!("{:?}", expanded.census),
                "census",
            ),
            (
                format!("{:?}", whole.edge_census),
                format!("{:?}", expanded.edge_census),
                "edge census",
            ),
            (
                format!("{:?}", whole.leaves_world),
                format!("{:?}", expanded.leaves_world),
                "leaves world",
            ),
            (
                format!("{:?}", whole.open_sea),
                format!("{:?}", expanded.open_sea),
                "open sea",
            ),
            (
                format!("{:?}", whole.fatal),
                format!("{:?}", expanded.fatal),
                "fatal",
            ),
            (
                format!("{:?}", whole.work),
                format!("{:?}", expanded.work),
                "work",
            ),
            (
                format!("{:?}", whole.unflown),
                format!("{:?}", expanded.unflown),
                "unflown",
            ),
            (
                format!("{:?}", whole.flown),
                format!("{:?}", expanded.flown),
                "flown table",
            ),
        ] {
            assert_eq!(
                a, b,
                "{name}: the summarised closure answers {what} otherwise"
            );
        }
        assert!(
            whole.nodes.iter().all(|(c, _)| !plain_here.contains(c)),
            "{name}: the summarised graph's node table holds a plain-sea cell"
        );
        let kept: Table = expanded
            .nodes
            .iter()
            .filter(|(c, _)| !plain_here.contains(c))
            .cloned()
            .collect();
        assert_eq!(
            whole.nodes, kept,
            "{name}: the node tables differ beyond the plain-sea cells"
        );
        let water_not_plain = r
            .states
            .iter()
            .filter(|(c, k)| {
                matches!(k, super::RestKind::Afloat | super::RestKind::Submerged)
                    && !plain_here.contains(c)
            })
            .count();
        plain += plain_here.len();
        boundary += water_not_plain;
        if r.sea_components >= 2 {
            two += 1;
        }
        eprintln!(
            "{name}: {} state(s), {} plain sea in {} component(s), {water_not_plain} water \
             state(s) not plain",
            r.states.len(),
            plain_here.len(),
            r.sea_components
        );
    }
    assert!(
        plain > 0 && boundary > 0 && two > 0,
        "the cases bind nothing: {plain} plain, {boundary} boundary, {two} world(s) of two \
         components"
    );
}

/// **A plain-sea cell is the one `expand` gives** (ADR-0037 §2): over every
/// cell of the window of each invariance world under an ocean, a cell
/// `plain_sea` holds for expands, through the general rule, to exactly
/// `plain_node` — submerged, six dives in cell order, nothing flown, nothing
/// flagged, nothing read; and a submerged cell it refuses expands to
/// something else (a climb out, a dive short, or a rim's open-sea flag).
/// Refuses a run with no plain cell and no refused submerged cell.
#[test]
fn a_plain_sea_cell_is_the_one_expand_gives() {
    let params = super::ReachParams::shipped(false);
    let (mut plain, mut refused, mut refused_otherwise) = (0usize, 0usize, 0usize);
    for (name, w, _) in invariance_worlds() {
        if !matches!(w.ambient, super::Ambient::Ocean(_)) {
            continue;
        }
        let memo = w.graph_memo(&params);
        let d = &memo.dense;
        for x in d.lo[0]..d.lo[0] + d.dims[0] {
            for y in d.lo[1]..d.lo[1] + d.dims[1] {
                for z in d.lo[2]..d.lo[2] + d.dims[2] {
                    let c = [x, y, z];
                    let e = w.expand(&memo, &params, c, 0, None);
                    if w.plain_sea(d, c) {
                        plain += 1;
                        assert_eq!(
                            e,
                            super::plain_node(c),
                            "{name}: the plain-sea cell {c:?} expands otherwise"
                        );
                    } else if e.kind == Some(super::RestKind::Submerged) {
                        refused += 1;
                        if e != super::plain_node(c) {
                            refused_otherwise += 1;
                        }
                    }
                }
            }
        }
    }
    eprintln!(
        "plain sea: {plain} cell(s) the rule holds for, {refused} submerged cell(s) it refuses, \
         {refused_otherwise} of them expanding to something else"
    );
    assert!(plain > 0 && refused_otherwise > 0, "the cases bind nothing");
    assert_eq!(
        refused, refused_otherwise,
        "a submerged cell the rule refuses expands to the six dives: the rule is too narrow"
    );
}

/// **The closure is the one every launch gives** (spec-0113 §4.7, ADR-0037
/// §2): the chunked closure, whose rounds fly only the launches a node's
/// envelope still asks for, reaches every state — and finds every way out of
/// the world — that flying every launch of every standing and holding state,
/// round after round until nothing new is reached, reaches; at chunk sides of
/// sixteen columns and of two. Red on any state or exit either way, and on
/// cases where flying everything reaches nothing the pruned rounds did not
/// fly, or where no holding state flew (either would leave the comparison
/// binding nothing: the ladder cliff is reached only through its holding
/// states, so a round that left them unflown is red here).
#[test]
fn the_chunked_closure_is_the_one_every_launch_gives() {
    let (mut states, mut pruned, mut worlds_n, mut holding) = (0usize, 0usize, 0usize, 0usize);
    for (name, w, roots) in invariance_worlds() {
        for side in [16, 2] {
            super::chunks::at_side(side, || {
                let mut r = w.body_reach(&roots);
                let closed: Vec<([i32; 3], super::RestKind)> = r.states.iter().collect();
                let exits: Vec<[i32; 3]> = r.leaves_world.iter().copied().collect();
                let unflown = r.states.keys().filter(|c| r.unflown(*c)).count();
                loop {
                    let nodes: Vec<[i32; 3]> = r.states.keys().filter(|c| r.unflown(*c)).collect();
                    if nodes.is_empty() {
                        break;
                    }
                    w.fly_cells(&mut r, &nodes);
                }
                let every: Vec<([i32; 3], super::RestKind)> = r.states.iter().collect();
                assert_eq!(
                    closed, every,
                    "{name} (side {side}): the chunked closure misses or adds a state every launch \
                     gives"
                );
                assert_eq!(
                    exits,
                    r.leaves_world.iter().copied().collect::<Vec<_>>(),
                    "{name} (side {side}): flying every launch finds a way out of the world the \
                     closure does not"
                );
                if side == 16 {
                    states += closed.len();
                    pruned += unflown;
                    holding += r.census()[3];
                    worlds_n += 1;
                }
            });
        }
    }
    eprintln!(
        "closure exactness: {worlds_n} world(s), {states} state(s), {holding} holding, {pruned} \
         state(s) with a launch the rounds left unflown"
    );
    assert!(
        states > 0 && pruned > 0 && holding > 0,
        "the cases bind nothing: no launch was left unflown, or no state was a holding one"
    );
}

/// A cell and its predecessors, each with its edge.
type Preds = ([i32; 3], Vec<([i32; 3], super::Edge)>);

/// What a plain queue walking the predecessor lists backwards from `seeds`
/// reaches — the walk the readers took before they read by chunk — with the
/// open sea joined when `sea`.
fn reaching_plainly(
    r: &super::Reach,
    seeds: &BTreeSet<[i32; 3]>,
    admit: impl Fn(&[i32; 3]) -> bool,
    sea: bool,
) -> BTreeSet<[i32; 3]> {
    let mut out = seeds.clone();
    let mut queue: std::collections::VecDeque<[i32; 3]> = seeds.iter().copied().collect();
    let mut joined = !sea;
    while let Some(cur) = queue.pop_front() {
        if !joined && r.open_sea.contains(&cur) {
            joined = true;
            for s in &r.open_sea {
                if out.insert(*s) {
                    queue.push_back(*s);
                }
            }
        }
        for (p, _) in r.preds(cur) {
            if admit(&p) && out.insert(p) {
                queue.push_back(p);
            }
        }
    }
    out
}

/// **A walk read reach chunk by reach chunk answers what a plain walk
/// answers** (ADR-0037 §2, the readers): over every invariance world, at
/// chunk sides of sixteen, seven and two columns, the backward walk the
/// boundary and leave proofs read ([`super::Reach::reaching`]) — from the
/// standing states with the open sea joined, and from the open sea through
/// wet states only — and the predecessor reading by chunk
/// ([`super::Reach::preds_by_chunk`]) equal a plain queue's answers. Red on
/// any cell either way.
#[test]
fn a_walk_by_chunk_answers_what_a_plain_walk_answers() {
    let (mut walked, mut sea_walked) = (0usize, 0usize);
    for (name, w, roots) in invariance_worlds() {
        for side in [16, 7, 2] {
            super::chunks::at_side(side, || {
                let r = w.body_reach(&roots);
                let standing = r.standing();
                let back = r.reaching(standing.iter().copied(), |_| true, true);
                assert_eq!(
                    back,
                    reaching_plainly(&r, &standing, |_| true, true),
                    "{name} (side {side}): the backward walk by chunk reaches otherwise"
                );
                let wet = |c: &[i32; 3]| {
                    matches!(
                        r.states.get(c),
                        Some(super::RestKind::Afloat | super::RestKind::Submerged)
                    )
                };
                let sea = r.reaching(r.open_sea.iter().copied(), wet, false);
                assert_eq!(
                    sea,
                    reaching_plainly(&r, &r.open_sea, wet, false),
                    "{name} (side {side}): the walk through the sea by chunk reaches otherwise"
                );
                let mut by_chunk: Vec<Preds> = Vec::new();
                r.preds_by_chunk(r.states.keys(), |c, ps| by_chunk.push((c, ps.to_vec())));
                by_chunk.sort_by_key(|(c, _)| *c);
                let plain: Vec<Preds> = r.states.keys().map(|c| (c, r.preds(c))).collect();
                assert_eq!(
                    by_chunk, plain,
                    "{name} (side {side}): the predecessors read by chunk differ"
                );
                if side == 2 {
                    walked += back.len();
                    sea_walked += sea.len();
                }
            });
        }
    }
    eprintln!("walks by chunk: {walked} cell(s) walked back, {sea_walked} through the sea");
    assert!(walked > 0 && sea_walked > 0, "the walks bind nothing");
}

/// **The learned route tree read by chunk is the tree one least-cost search
/// gives** (ADR-0037 §2, the readers): over every invariance world, from its
/// root and from its standing states with an odd coordinate sum as a route,
/// at chunk sides of sixteen, seven and two columns, the tree and the order
/// `nav::learned::route_tree` answers equal those of one search over the
/// whole closure in (cost, cell) order. Red on any predecessor or position.
#[test]
fn the_route_tree_by_chunk_is_the_one_search_gives() {
    let (mut cells, mut joined) = (0usize, 0usize);
    for (name, w, roots) in invariance_worlds() {
        for side in [16, 7, 2] {
            super::chunks::at_side(side, || {
                let r = w.body_reach(&roots);
                let odd: Vec<[i32; 3]> = r
                    .standing()
                    .into_iter()
                    .filter(|c| (c[0] + c[1] + c[2]).rem_euclid(5) == 0)
                    .collect();
                for route in [roots.clone(), odd] {
                    let chunked = crate::compiler::nav::learned::route_tree(&r, &route);
                    let plain = crate::compiler::nav::learned::route_tree_plainly(&r, &route);
                    assert_eq!(
                        chunked, plain,
                        "{name} (side {side}): the route tree read by chunk differs"
                    );
                    if side == 2 {
                        cells += chunked.1.len();
                        joined += chunked.0.values().filter(|p| p.is_some()).count();
                    }
                }
            });
        }
    }
    eprintln!("route trees by chunk: {cells} cell(s) ordered, {joined} with a predecessor");
    assert!(cells > 0 && joined > 0, "the route trees bind nothing");
}

/// **The portal-edge count is what crosses a chunk face** (ADR-0037 §2): over
/// every invariance world, at chunk sides of sixteen, seven and two columns,
/// the portal edges a closure states equal the edges of its own successor
/// lists whose two ends lie in different chunks of that side, and its face
/// cells the distinct cells those edges land on — counted here by reading the
/// lists back, a second method that shares nothing with the count taken as
/// the edges were joined. Red on any difference; perturbed by miscounting a
/// portal edge in [`super::chunks::Visits::joined`].
#[test]
fn the_portal_edges_are_the_edges_that_cross_a_chunk_face() {
    let (mut portals, mut sides_moved) = (0u64, 0usize);
    for (name, w, roots) in invariance_worlds() {
        let mut by_side = Vec::new();
        for side in [16, 7, 2] {
            super::chunks::at_side(side, || {
                let r = w.body_reach(&roots);
                let key = |c: [i32; 3]| (c[0].div_euclid(side), c[2].div_euclid(side));
                let mut crossing = 0u64;
                let mut faces: BTreeSet<[i32; 3]> = BTreeSet::new();
                // A plain-sea state's dives are computed, never joined: no
                // list holds them and no portal counts them (ADR-0037 §2).
                for c in r.states.keys().filter(|c| !r.states.plain(c)) {
                    for e in r.succ(c) {
                        if key(c) != key(e.to) {
                            crossing += 1;
                            faces.insert(e.to);
                        }
                    }
                }
                assert_eq!(
                    r.portals,
                    [crossing, faces.len() as u64],
                    "{name} (side {side}): the stated portal edges and face cells are not the \
                     ones the lists cross"
                );
                portals += crossing;
                by_side.push(crossing);
            });
        }
        if by_side.windows(2).any(|p| p[0] != p[1]) {
            sides_moved += 1;
        }
    }
    eprintln!(
        "portal edges: {portals} over the invariance worlds at three sides; the count moved with \
         the side on {sides_moved} world(s)"
    );
    assert!(portals > 0 && sides_moved > 0, "the cases bind nothing");
}

/// **One answer to "is this cell the sea"**: the rest relation's water
/// (`World::is_water` or `World::ambient_water`) and the reach snapshot the
/// launches fly through (`Dense`, its `WATER` bit) are asked every cell of
/// generated ocean worlds, red on any difference. Each world is noise in and
/// around the sea's band (y 53..=66 over x, z 0..=11, sea level 62 over a
/// floor at 54): every cell a solid, a 1.5-tall barrier, a fence gate,
/// authored water, lava or air, with one placed piece's box and one killing
/// volume laid over it. A killing volume is no water to the rest relation
/// and is not carried by the snapshot, so its cells are held to "no water,
/// no rest state" instead. And the general form of the ground-in-the-band
/// defect: no solid or 1.5-tall cell is a rest state, and the closure from a
/// platform over the sea rests a body in none of them. Refuses a run whose
/// cases hold no solid in the band outside the box, or whose closures reach
/// no sea. Perturbation: ambient water judged by the band and the pieces'
/// boxes alone → red (a solid outside the box is water to the rest relation
/// and stone to the snapshot).
#[test]
fn the_sea_the_launches_read_is_the_sea_the_rest_relation_reads() {
    use super::{Dense, WATER};
    let mut band_ground = 0usize;
    let mut compared = 0usize;
    let mut water = 0usize;
    let mut sea_states = 0usize;
    for seed in 0..16u64 {
        let mut r = Lcg(seed);
        let mut solid = BTreeSet::new();
        let mut tall = BTreeSet::new();
        let mut gates = BTreeSet::new();
        let mut flooded = BTreeSet::new();
        let mut lava = BTreeSet::new();
        for x in 0..12 {
            for z in 0..12 {
                for y in 53..=66 {
                    let c = [x, y, z];
                    match r.next(12) {
                        0..=2 => {
                            solid.insert(c);
                        }
                        3 => {
                            tall.insert(c);
                        }
                        4 => {
                            gates.insert(c);
                        }
                        5 => {
                            flooded.insert(c);
                        }
                        6 => {
                            flooded.insert(c);
                            lava.insert(c);
                        }
                        _ => {}
                    }
                }
            }
        }
        // A platform over the sea, clear over it, for the closure's root.
        for x in 4..=6 {
            for z in 4..=6 {
                solid.insert([x, 67, z]);
                for y in 68..=70 {
                    for s in [&mut solid, &mut tall, &mut gates, &mut flooded] {
                        s.remove(&[x, y, z]);
                    }
                }
            }
        }
        let corner = |r: &mut Lcg| [r.next(9), 53 + r.next(10), r.next(9)];
        let lo = corner(&mut r);
        let built = vec![("piece".to_string(), (lo, [lo[0] + 3, lo[1] + 3, lo[2] + 3]))];
        let lo = corner(&mut r);
        let mut p = Premises::geometry_only();
        p.ambient = super::Ambient::Ocean(crate::compiler::nav::Sea {
            level: 62,
            floor_top: 54,
        });
        p.built = built;
        p.lethal_regions = vec![("pit".to_string(), (lo, [lo[0] + 2, lo[1] + 2, lo[2] + 2]))];
        let w = World::from_occupancy(
            crate::compiler::assembled::Occupancy {
                solid: solid.into(),
                tall: tall.into(),
                use_gates: gates.into(),
                flooded: flooded.into(),
                partial: Default::default(),
                waterloggable: Default::default(),
                lava: lava.into(),
                climb: Default::default(),
                unheld_climb: Default::default(),
                material: Default::default(),
                footprints: Default::default(),
                hiding: Default::default(),
                hurting: Default::default(),
                leaves: Default::default(),
            },
            p,
        );
        let d = Dense::of(&w);
        for x in -2..14 {
            for z in -2..14 {
                for y in 50..=72 {
                    let c = [x, y, z];
                    let rest = w.is_water(c) || w.ambient_water(c);
                    let blocked = w.solid.contains(&c) || w.tall.contains(&c);
                    if blocked {
                        assert_eq!(w.rest_kind(c), None, "seed {seed}: {c:?} rests a body");
                        if (55..=62).contains(&y) && !w.is_built(c) {
                            band_ground += 1;
                        }
                    }
                    if w.lethal.contains(&c) {
                        assert!(!rest, "seed {seed}: the killing volume at {c:?} is water");
                        continue;
                    }
                    let snap = d.code(c) & WATER != 0;
                    assert_eq!(
                        rest, snap,
                        "seed {seed}: {c:?} is water to the rest relation: {rest}, to the snapshot: {snap}"
                    );
                    compared += 1;
                    water += usize::from(rest);
                }
            }
        }
        let reach = w.body_reach(&[[5, 68, 5]]);
        for c in reach.states.keys() {
            assert!(
                !w.solid.contains(&c) && !w.tall.contains(&c),
                "seed {seed}: the closure rests a body in the block at {c:?}"
            );
        }
        sea_states += reach
            .states
            .values()
            .filter(|k| matches!(k, super::RestKind::Afloat | super::RestKind::Submerged))
            .count();
    }
    eprintln!(
        "sea agreement: {compared} cell(s) compared, {water} water; {band_ground} solid or \
         tall cell(s) in the band outside the box; {sea_states} sea state(s) reached"
    );
    assert!(
        band_ground > 0,
        "the cases bind nothing: no ground in the band"
    );
    assert!(
        sea_states > 0,
        "the cases bind nothing: no closure reaches the sea"
    );
}
