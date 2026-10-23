//! **One answer to "can the body reach this cell"**: the reach graph and every
//! consumer that counts reached cells asked the same generated cases, red on
//! any difference (CLAUDE.md, "One question, one implementation").
//!
//! The consumers are the blockout battery's place reachability (`DW0837`,
//! [`crate::compiler::blockout::reached_from`]), its unallocated-crossing
//! flood (`DW0838`, [`crate::compiler::blockout::crossing_flood`]), the moves
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
//! [`crate::compiler::view::showing::survey`] reports).
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
//! a held cell, or no per-piece reader's floor needs a jump or a fall.

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
        let (_, light_entry, light) = crate::admit::light::floor_measured(&wide, &BTreeSet::new());
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
        let (_, _, surveyed) = crate::compiler::view::showing::floor_reached(&st, &st_cells);
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
        let (_, _, doored) = crate::admit::light::floor_measured(&model, &roots);
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
         {floor_needed} per-piece floor(s) a jump or fall"
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
}
