//! **The long gallery** (spec-0086): a synthesised corridor of identical bays,
//! shared by the loop tests (`tests/endless_corridor.rs`) and the remedy rows
//! that take DW0945–DW0948's moves (`tests/remedy_reachability.rs`). Every
//! fixture built over it is `tests/fixtures/long-gallery` plus one edit.
//!
//! The piece is synthesised rather than drawn from the content library: the
//! property under test is "a corridor that repeats exactly, and whose view
//! closes inside one bay", and a piece that holds it by construction is the only
//! one whose geometry a reader can check from this file.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// `7 x 7 x 43`: a floor three courses thick (y 0..2, so a body stands at y 3),
/// an interior three tall (y 3..5) and a roof at y 6. Across x: the west wall at
/// 0, the passage 1..3, the east wall at 4 with a glass window in every bay, a
/// sealed cavity strip at 5 behind the windows, and the outer wall at 6.
pub const SIZE: [i32; 3] = [7, 7, 43];

/// The y a body stands at.
pub const FLOOR_Y: i32 = 3;

/// How many cells one bay repeats over, along z.
pub const PERIOD: i32 = 6;

/// The z the first bay starts at.
pub const FIRST_BAY: i32 = 4;

/// How many bays the corridor has.
pub const BAYS: i32 = 6;

/// The z of bay `k`'s first course — its mouth, where a slab can stand.
pub fn bay(k: i32) -> i32 {
    FIRST_BAY + PERIOD * k
}

/// What one fixture varies about the corridor's blocks.
#[derive(Clone, Debug, Default)]
pub struct Cuts {
    /// Cells forced to air (a removed wall, a removed roof cell, a removed lamp).
    pub air: Vec<[i32; 3]>,
    /// Cells forced to a block (a lamp behind a window, a floor of magma).
    pub blocks: Vec<([i32; 3], &'static str)>,
}

/// Every non-air cell of the corridor, with `cuts` applied last.
pub fn gallery_cells(cuts: &Cuts) -> Vec<([i32; 3], &'static str)> {
    let [sx, sy, sz] = SIZE;
    let mut m: std::collections::BTreeMap<[i32; 3], &'static str> =
        std::collections::BTreeMap::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let c = [x, y, z];
                let shell =
                    y < FLOOR_Y || y == sy - 1 || z == 0 || z == sz - 1 || x == 0 || x == sx - 1;
                if shell {
                    m.insert(c, "minecraft:stone_bricks");
                    continue;
                }
                // The east wall, and the cavity strip behind it: the cavity is
                // open only alongside the bays.
                let in_bays = (bay(0)..bay(BAYS)).contains(&z);
                if x == 4 {
                    let o = (z - FIRST_BAY).rem_euclid(PERIOD);
                    if in_bays && o == 3 && y == FLOOR_Y + 1 {
                        m.insert(c, "minecraft:glass");
                    } else {
                        m.insert(c, "minecraft:stone_bricks");
                    }
                    continue;
                }
                if x == 5 {
                    if !in_bays {
                        m.insert(c, "minecraft:stone_bricks");
                    }
                    continue;
                }
                // The passage.
                if in_bays {
                    let o = (z - FIRST_BAY).rem_euclid(PERIOD);
                    let baffle = (o == 2 && (x == 1 || x == 2)) || (o == 5 && (x == 2 || x == 3));
                    if baffle {
                        m.insert(c, "minecraft:stone_bricks");
                    } else if o == 1 && x == 2 && y == sy - 2 {
                        m.insert(c, "minecraft:lantern[hanging=true]");
                    }
                }
            }
        }
    }
    // The ends are lit too, so the light gate is not what any fixture is about.
    m.insert([2, sy - 2, 2], "minecraft:lantern[hanging=true]");
    m.insert([2, sy - 2, sz - 2], "minecraft:lantern[hanging=true]");
    for c in &cuts.air {
        m.remove(c);
    }
    for (c, b) in &cuts.blocks {
        m.insert(*c, b);
    }
    m.into_iter().collect()
}

/// The anchors, piece-local. `anchor/slab` is the mouth of bay 3, mid-height,
/// so `± [1, 1, 0]` is exactly the passage's open cross-section; `anchor/landing`
/// is the mouth of bay 2, one period back. `anchor/west-wall` centres a box over
/// the whole west wall of the bays, and `anchor/west-wall-landing` one over bay
/// 2's west wall alone.
pub fn gallery_anchors() -> Value {
    let mid = FLOOR_Y + 1;
    json!({
        "spawn": { "pos": [2, FLOOR_Y, 2], "facing": "south", "role": "entry" },
        "anchor/porch": { "pos": [2, FLOOR_Y, 3] },
        "anchor/slab": { "pos": [2, mid, bay(3)] },
        "anchor/landing": { "pos": [2, mid, bay(2)] },
        "anchor/two-back": { "pos": [2, mid, bay(1)] },
        "anchor/end": { "pos": [2, FLOOR_Y, SIZE[2] - 2] },
        "anchor/bell": { "pos": [1, FLOOR_Y, SIZE[2] - 2] },
        "anchor/west-wall": { "pos": [0, mid, (bay(0) + bay(BAYS) - 2) / 2] },
        "anchor/west-wall-landing": { "pos": [0, mid, bay(2) + 2] },
        "anchor/in-the-hall": { "pos": [3, FLOOR_Y, bay(2) + 3] },
        "anchor/in-the-slab": { "pos": [3, FLOOR_Y, bay(3)] },
    })
}

/// Write the corridor into a fresh prefab directory under the test's temp
/// root, and return it.
pub fn gallery_prefabs(tag: &str, cuts: &Cuts) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("long-gallery-prefabs-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    super::write_single_prefab(
        &dir,
        "long-gallery",
        SIZE,
        &gallery_cells(cuts),
        gallery_anchors(),
    );
    dir
}

/// The world cell of a piece-local cell, given the anchor table the build
/// resolved — read off `critical-path.json`'s own loop step rather than assumed.
pub fn origin_from_step(step: &Value) -> [i64; 3] {
    let cross = step["cross"].as_array().unwrap();
    let v = |i: usize| cross[i].as_i64().unwrap();
    [
        v(0) - 2,
        v(1) - i64::from(FLOOR_Y),
        v(2) - i64::from(bay(3)),
    ]
}
