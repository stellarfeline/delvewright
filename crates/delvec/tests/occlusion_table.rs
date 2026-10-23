//! **What the eye looks through** (spec-0118 criterion 17,
//! `docs/reference/sight-through-blocks.md`): `World::hides` reads the pinned
//! jar's occlusion table, `crates/dsl/data/occlusion-1.21.11.tsv`, written by
//! `tools/maintenance/dump-block-light.py` — never a list in a module and
//! never the collision class.

use std::collections::BTreeSet;

use delvec::compiler::nav::{Premises, World};
use delvewright_dsl::blockshape::hides;

/// The control set the spec names, read through the world as a sight walk
/// reads it: one block per cell, classified by the one occupancy model.
#[test]
fn the_control_set_hides_and_is_looked_through_as_the_jar_says() {
    let hiding = [
        "minecraft:stone",
        "minecraft:stone_bricks",
        "minecraft:polished_deepslate",
        "minecraft:stripped_dark_oak_log[axis=y]",
    ];
    let looked_through = [
        "minecraft:iron_bars",
        "minecraft:glass",
        "minecraft:glass_pane",
        "minecraft:campfire",
        "minecraft:oak_fence",
        "minecraft:stone_brick_wall",
        "minecraft:oak_stairs",
        "minecraft:oak_slab",
        "minecraft:oak_door",
        "minecraft:iron_trapdoor",
        "minecraft:oak_leaves",
        "minecraft:tinted_glass",
    ];
    let cells: Vec<([i32; 3], &str)> = hiding
        .iter()
        .chain(looked_through.iter())
        .enumerate()
        .map(|(i, b)| ([i32::try_from(i).unwrap() * 2, 64, 0], *b))
        .collect();
    let map = cells.iter().map(|(c, b)| (*c, (*b).to_string())).collect();
    let w = World::from_occupancy(
        delvec::compiler::assembled::occupancy_of(map, &BTreeSet::new()),
        Premises::geometry_only(),
    );
    for (c, b) in &cells {
        let expect = hiding.contains(b);
        assert_eq!(hides(b), expect, "{b}: the table");
        assert_eq!(w.hides(*c), expect, "{b}: the world");
    }
    // The collision answer calls a grille, a pane and a fence a wall to a
    // camera; the eye looks through each.
    for (c, b) in cells.iter().filter(|(_, b)| {
        matches!(
            *b,
            "minecraft:iron_bars"
                | "minecraft:glass_pane"
                | "minecraft:oak_fence"
                | "minecraft:glass"
        )
    }) {
        assert!(w.blocks_camera(*c) && !w.hides(*c), "{b}");
    }
    assert!(!hides("minecraft:air"));
    assert!(delvewright_dsl::blockshape::occlusion_row_count() > 1000);
}

/// The one known divergence from light opacity: tinted glass is light-opaque
/// in vanilla (`filterLight` 15 in the vendored minecraft-data table, measured
/// in `docs/reference/sight-through-blocks.md` §3) and looked through, so the
/// sight table is never read from light opacity. A double slab is a full
/// opaque cube of a slab block and hides; a bottom slab does not.
#[test]
fn tinted_glass_is_looked_through_and_a_double_slab_hides() {
    assert!(!hides("minecraft:tinted_glass"));
    assert!(hides("minecraft:oak_slab[type=double]"));
    assert!(!hides("minecraft:oak_slab[type=bottom]"));
    assert!(
        !hides("minecraft:oak_slab"),
        "a bare slab is its default, the bottom"
    );
}
