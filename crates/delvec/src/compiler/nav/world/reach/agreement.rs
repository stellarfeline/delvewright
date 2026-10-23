//! **One answer to "can the body reach this cell"**: the reach graph and every
//! consumer that counts reached cells asked the same generated cases, red on
//! any difference (CLAUDE.md, "One question, one implementation").
//!
//! The consumers are the blockout battery's place reachability (`DW0837`,
//! [`crate::compiler::blockout::reached_from`]) and the piece contract's
//! `contract-reachability` walk ([`crate::grammar::contract::reached_shut`]).
//! The cases are parkour courses: platforms three wide along `x` with gaps of
//! zero to four columns, rises of up to two courses either way, a low ceiling
//! that forbids a jump, a pit under the gaps, and declared jumps across some
//! gaps. The battery is asked over the world with its declared jumps; a piece
//! contract has no declared-jump surface, so it is asked over the same blocks
//! without them and compared with the graph over those blocks.
//!
//! The test also reds when its cases are vacuous: when no case needs a jump of
//! the launch set, or no case needs a declared jump, for the graph to reach
//! more than the walk does.

use super::super::{Premises, World};
use crate::grammar::block::BlockState;
use crate::grammar::geom::Box3;
use crate::grammar::model::VoxelModel;
use crate::schem::prefab::{ContractEdge, ContractSpace, Region, SpatialContract};
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

/// A contract whose confinement never binds: the start column `x = 1` is the
/// entry space, everything else one more, joined by a walk and a drop each way.
fn open_contract(case: &Case) -> SpatialContract {
    let [sx, sy] = case.size;
    let (xmax, ymax) = (sx as i32 - 1, sy as i32 - 1);
    let mut spaces = BTreeMap::new();
    for (name, from, to) in [
        ("start", [0, 0, 0], [1, ymax, 4]),
        ("rest", [2, 0, 0], [xmax, ymax, 4]),
    ] {
        spaces.insert(
            name.to_string(),
            ContractSpace {
                envelope: "open".to_string(),
                boxes: vec![region(from, to)],
            },
        );
    }
    SpatialContract {
        entry: "start".to_string(),
        spaces,
        no_body: BTreeMap::new(),
        edges: vec![
            edge("start", "rest", "walk"),
            edge("start", "rest", "drop"),
            edge("rest", "start", "drop"),
        ],
        faces: Vec::new(),
        no_body_majority_ack: None,
    }
}

#[test]
fn the_battery_and_the_contract_reach_what_the_reach_graph_reaches() {
    const CASES: u64 = 48;
    let mut launch_needed = 0usize;
    let mut declared_needed = 0usize;
    let mut contract_launch_needed = 0usize;
    let mut declared = 0usize;
    let mut cells = 0usize;
    for seed in 0..CASES {
        let case = generate(seed);
        declared += case.jumps.len();
        let bare = World::from_solid_cells(case.solid.clone());
        let mut p = Premises::geometry_only();
        p.declared_jumps = case.jumps.clone();
        let jumping = bare.with_premises(p);

        // The battery, over the world with its declared jumps.
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

        // The contract, over the same blocks: a piece declares no jump.
        let model = model_of(&case);
        let (targets, reached) =
            crate::grammar::contract::reached_shut(&model, &open_contract(&case));
        let starts: Vec<[i32; 3]> = targets.iter().filter(|c| c[0] <= 1).copied().collect();
        assert!(!starts.is_empty(), "case {seed}: the entry space has floor");
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
    }
    eprintln!(
        "reach agreement: {CASES} case(s), {cells} reached cell(s), {declared} declared jump(s); \
         {launch_needed} case(s) need a launch-set jump, {declared_needed} a declared jump, \
         {contract_launch_needed} of the contract's a launch-set jump"
    );
    assert!(
        launch_needed > 0 && contract_launch_needed > 0,
        "no case needs a jump of the launch set: the agreement is vacuous"
    );
    assert!(
        declared_needed > 0,
        "no case needs a declared jump: the agreement is vacuous"
    );
}
