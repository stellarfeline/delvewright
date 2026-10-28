//! **The ballistic instrument** (spec-0113 §8.2): the simulator held to the
//! pinned jar's own movement code with no player input.
//!
//! Four bodies of an armour stand's box are thrown over a small rig — stone, slime,
//! water and a slab lip — and each tick's position is what the simulator
//! ([`super::simulate`], [`Rules::Game`]) says a body with no input does there.
//! The one-time instrument `delvec::compiler::ballistics` — a PackTest written
//! by hand, never by a build or CI — builds the same rig on the pinned server, summons `minecraft:armor_stand` bodies with the same
//! `Motion` — a living body the game moves with no input (a mob with `NoAI`
//! is not moved at all: measured, the body summoned with `Motion` stays where
//! it was put), and asserts every tick's `Pos` against these trajectories within
//! [`TOLERANCE`]. The trajectories are committed beside the case file
//! (`crates/dsl/tests/fixtures/movement/ballistics-1.21.11.json`) and a test
//! refuses when the simulator no longer computes them. The measurement and the
//! revision it was taken at are in `docs/reference/body-movement.md`.
//!
//! Coordinates are relative to the rig's origin cell, a block corner.

use super::{Aabb, Attributes, CollisionWorld, Fluid, Input, Material, Rules, State, simulate};
use std::collections::BTreeMap;

/// The ticks each body is read over.
pub const TICKS: usize = 60;

/// The tolerance a `Pos` reading is held to, per axis.
pub const TOLERANCE: f64 = 1e-4;

/// The armour stand's box (`EntityType.ARMOR_STAND`, `sized(0.5F, 1.975F)`,
/// cited).
pub const ARMOR_STAND_BOX: (f64, f64) = (0.5, 1.975);

/// A block of the rig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RigBlock {
    /// `minecraft:stone`.
    Stone,
    /// `minecraft:slime_block`.
    Slime,
    /// A still `minecraft:water` source.
    Water,
    /// `minecraft:stone_slab[type=bottom]`.
    BottomSlab,
}

impl RigBlock {
    /// The block state the rig is built of.
    #[must_use]
    pub fn state(self) -> &'static str {
        match self {
            RigBlock::Stone => "minecraft:stone",
            RigBlock::Slime => "minecraft:slime_block",
            RigBlock::Water => "minecraft:water",
            RigBlock::BottomSlab => "minecraft:stone_slab[type=bottom]",
        }
    }
}

/// One thrown body: where it is summoned and the `Motion` it is given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Throw {
    /// What it measures.
    pub name: &'static str,
    /// Feet centre at summon, relative to the rig's origin.
    pub start: [f64; 3],
    /// `Motion` at summon.
    pub motion: [f64; 3],
}

/// The four bodies, one per lane four blocks apart along `x`.
pub const THROWS: [Throw; 4] = [
    // Thrown up and forward over stone: the arc, the landing, the slide.
    Throw {
        name: "stone",
        start: [0.5, 1.0, 0.5],
        motion: [0.0, 0.42, 0.3],
    },
    // Dropped four blocks onto slime: the bounce.
    Throw {
        name: "slime",
        start: [4.5, 5.0, 0.5],
        motion: [0.0, 0.0, 0.05],
    },
    // Dropped into a pool four deep: the water's drag and the sink.
    Throw {
        name: "water",
        start: [8.5, 6.0, 1.5],
        motion: [0.0, 0.0, 0.0],
    },
    // Dropped onto a bottom slab and slid along it: a landing on a partial
    // block's top, and the friction of the slab it slides on.
    Throw {
        name: "slab",
        start: [12.5, 4.0, 4.5],
        motion: [0.0, 0.0, 0.25],
    },
];

/// The rig's blocks, relative to its origin; every other cell of the box
/// `[-1, -1, -1]..=[14, 7, 9]` is air.
#[must_use]
pub fn rig() -> BTreeMap<[i32; 3], RigBlock> {
    let mut m = BTreeMap::new();
    let mut fill = |lo: [i32; 3], hi: [i32; 3], b: RigBlock| {
        for x in lo[0]..=hi[0] {
            for y in lo[1]..=hi[1] {
                for z in lo[2]..=hi[2] {
                    m.insert([x, y, z], b);
                }
            }
        }
    };
    // The stone lane.
    fill([-1, 0, -1], [1, 0, 8], RigBlock::Stone);
    // The slime lane: stone, slime where the body falls.
    fill([3, 0, -1], [5, 0, 8], RigBlock::Stone);
    fill([3, 0, -1], [5, 0, 2], RigBlock::Slime);
    // The pool: a stone basin, water four deep inside it.
    fill([6, -1, -1], [10, 4, 3], RigBlock::Stone);
    fill([7, 0, 0], [9, 4, 2], RigBlock::Water);
    // The slab lane: stone, a course of bottom slabs across it.
    fill([11, 0, -1], [13, 0, 8], RigBlock::Stone);
    fill([11, 1, 2], [13, 1, 8], RigBlock::BottomSlab);
    m
}

/// The extent of the rig's box, relative to its origin (what the template
/// clears to air before it builds).
pub const RIG_BOX: ([i32; 3], [i32; 3]) = ([-1, -1, -1], [14, 7, 9]);

struct RigWorld(BTreeMap<[i32; 3], RigBlock>);

impl CollisionWorld for RigWorld {
    fn shapes(&self, c: [i32; 3], out: &mut Vec<Aabb>) {
        let (x, y, z) = (f64::from(c[0]), f64::from(c[1]), f64::from(c[2]));
        match self.0.get(&c) {
            Some(RigBlock::Stone | RigBlock::Slime) => {
                out.push(Aabb::new(x, y, z, x + 1.0, y + 1.0, z + 1.0));
            }
            Some(RigBlock::BottomSlab) => out.push(Aabb::new(x, y, z, x + 1.0, y + 0.5, z + 1.0)),
            Some(RigBlock::Water) | None => {}
        }
    }
    fn material(&self, c: [i32; 3]) -> Material {
        if self.0.get(&c) == Some(&RigBlock::Slime) {
            Material::Slime
        } else {
            Material::Plain
        }
    }
    fn fluid(&self, c: [i32; 3]) -> Option<Fluid> {
        (self.0.get(&c) == Some(&RigBlock::Water)).then_some(Fluid::SOURCE_WATER)
    }
}

/// The body each throw is: the living body's attributes in an armour stand's
/// box, with the armour stand's `step_height` of `0`
/// (`ArmorStand.createAttributes`, cited; measured: a stand slid into a slab's
/// face stops at it).
#[must_use]
pub fn body() -> Attributes {
    let mut a = Attributes::PLAYER.with_box(ARMOR_STAND_BOX.0, ARMOR_STAND_BOX.1);
    a.step_height = 0.0;
    a
}

/// Each throw's position after every one of its [`TICKS`] ticks, in
/// [`THROWS`] order, as the simulator computes it.
#[must_use]
pub fn trajectories() -> Vec<Vec<[f64; 3]>> {
    let world = RigWorld(rig());
    let body = body();
    THROWS
        .iter()
        .map(|t| {
            let mut start = State::standing(t.start, &body);
            start.on_ground = false;
            start.vel = t.motion;
            simulate(&world, &body, Rules::Game, start, |n, _| {
                (n < TICKS).then(|| Input {
                    magnitude: 1.0,
                    ..Input::default()
                })
            })
            .iter()
            .map(|s| s.pos)
            .collect()
        })
        .collect()
}
