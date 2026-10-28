//! **A door a hand opens is walked through** (spec-0123 §3.1, criterion 17).
//!
//! Two rooms share a wall with one doorway; the doorway holds a closed door.
//! The route proof's router crosses a closed `oak_door` — the body opens it —
//! and refuses the same doorway when it holds a closed `iron_door`, which only
//! redstone moves. The passable set is the pinned registry's, and the binding
//! line states it.

use std::collections::{BTreeMap, BTreeSet};

use delvec::compiler::assembled::occupancy_of;
use delvec::compiler::nav::{Premises, World};

/// Floor at `y = 64` over `x 0..7, z 0..3`, a ceiling at `y = 67`, a dividing
/// wall at `x = 3` with its doorway at `z = 1` holding `door` (both halves).
fn two_rooms(door: &str) -> World {
    let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
    for x in -1..=7 {
        for z in -1..=3 {
            blocks.insert([x, 64, z], "minecraft:stone".into());
            blocks.insert([x, 67, z], "minecraft:stone".into());
            if x == -1 || x == 7 || z == -1 || z == 3 {
                for y in 65..=66 {
                    blocks.insert([x, y, z], "minecraft:stone".into());
                }
            }
        }
    }
    for z in 0..=2 {
        for y in 65..=66 {
            blocks.insert([3, y, z], "minecraft:stone".into());
        }
    }
    blocks.insert(
        [3, 65, 1],
        format!("minecraft:{door}[facing=east,half=lower,hinge=left,open=false,powered=false]"),
    );
    blocks.insert(
        [3, 66, 1],
        format!("minecraft:{door}[facing=east,half=upper,hinge=left,open=false,powered=false]"),
    );
    World::from_occupancy(
        occupancy_of(blocks, &BTreeSet::new()),
        Premises::geometry_only(),
    )
}

#[test]
fn the_route_crosses_a_closed_oak_door_and_not_a_closed_iron_one() {
    let (start, goal) = ([1, 65, 1], [5, 65, 1]);
    let oak = two_rooms("oak_door");
    let path = oak
        .find_path(start, goal)
        .expect("a body opens the oak door and walks through");
    assert!(
        path.contains(&[3, 65, 1]),
        "the route crosses the doorway: {path:?}"
    );
    let iron = two_rooms("iron_door");
    assert_eq!(
        iron.find_path(start, goal),
        None,
        "a closed iron door is a barrier to a hand"
    );
}

#[test]
fn the_binding_line_counts_the_registry() {
    let doors = delvewright_dsl::blockshape::doors_by_hand();
    let family = delvewright_dsl::blocks::BlockRegistry::v1_21_11()
        .ids()
        .filter(|id| {
            id.ends_with("_door") || id.ends_with("_trapdoor") || id.ends_with("_fence_gate")
        })
        .count();
    assert_eq!(
        doors.line(),
        format!("doors by hand: {} block(s) passable, 2 held", family - 2)
    );
}
