//! `tk-wall-walk` (L1) — the exterior parapet between the gatehouse roof and the
//! courtyard.
//!
//! Open sky the whole way: the piece exists so the **TEACH ambush** is fair by
//! sightline alone (dossier §4.3 — fairness is silhouette, never a telegraph).
//! The ambush turret is a roofless nook cut into the west parapet, so whatever
//! stands in its doorway is backlit by open sky and readable from the far end of
//! the run by anyone who looks before committing.
//!
//! Structurally it is a solid curtain wall: the mass below the walk is filled to
//! the keep datum (local y=10), so the parapet is a real wall rather than a
//! bridge, and no walkable cell ever borders the void — the merlon mass at both
//! piece edges bounds the walk envelope.

use crate::common::*;

pub const SX: i32 = 16;
pub const SY: i32 = 16;
pub const SZ: i32 = 34;

/// The walkable lane between the two parapets.
const LANE_X0: i32 = 3;
const LANE_X1: i32 = 12;
/// Ambush turret (roofless nook cut into the west parapet).
const NOOK_Z0: i32 = 15;
const NOOK_Z1: i32 = 17;

pub fn build(g: &mut Grid, seed: u64) {
    // ---- 1. The curtain-wall mass ------------------------------------------
    for x in 0..SX {
        for z in 0..SZ {
            for y in 0..=KEEP_FLOOR_Y {
                let name = if y < SHORE_FLOOR_Y + 4 {
                    pick(&plinth(), value_noise(seed, x, y, z, 0.14, 11))
                } else {
                    pick(&keep_wall(), value_noise(seed, x, y, z, 0.12, 13))
                };
                g.blk(x, y, z, name, None);
            }
            // the outer merlon mass bounds the walk at both piece edges
            if x <= 1 || x >= SX - 2 {
                for y in (KEEP_FLOOR_Y + 1)..=(KEEP_FLOOR_Y + 3) {
                    g.blk(
                        x,
                        y,
                        z,
                        pick(&keep_wall(), value_noise(seed, x, y, z, 0.2, 15)),
                        None,
                    );
                }
            }
        }
    }
    // walk surface
    g.fill_pal(
        bx(LANE_X0, LANE_X1, KEEP_FLOOR_Y, KEEP_FLOOR_Y, 0, SZ - 1),
        &keep_floor(),
        seed,
        0.22,
        17,
    );

    // ---- 2. Crenellations ---------------------------------------------------
    // Merlon / embrasure alternation on both inner parapets. An embrasure keeps a
    // `stone_brick_wall` course: the nav model treats walls as 1.5-tall barriers
    // you can neither pass nor stand on, so a player can look through the gap and
    // never fall out of it.
    for z in 0..SZ {
        for x in [2, SX - 3] {
            if z % 3 == 0 {
                g.blk(x, KEEP_WALK, z, "minecraft:stone_brick_wall", None);
            } else {
                for y in KEEP_WALK..=(KEEP_WALK + 2) {
                    g.blk(
                        x,
                        y,
                        z,
                        pick(&keep_wall(), value_noise(seed, x, y, z, 0.25, 19)),
                        None,
                    );
                }
            }
        }
    }
    // a collapsed merlon run (the keep has been shelled by the sea, not kept)
    g.carve(bx(2, 2, KEEP_WALK, KEEP_WALK + 2, 24, 27));
    for z in 24..=27 {
        g.blk(2, KEEP_WALK, z, "minecraft:stone_brick_wall", None);
        g.blk(3, KEEP_FLOOR_Y, z, "minecraft:cobblestone", None);
    }

    // ---- 3. The ambush turret (roofless, sky behind the doorway) ------------
    g.carve(bx(1, 2, KEEP_WALK, KEEP_WALK + 3, NOOK_Z0, NOOK_Z1));
    g.fill_pal(
        bx(1, 2, KEEP_FLOOR_Y, KEEP_FLOOR_Y, NOOK_Z0, NOOK_Z1),
        &keep_floor(),
        seed,
        0.3,
        21,
    );
    // the jambs stay, so the opening reads as a doorway rather than a gap
    for y in KEEP_WALK..=(KEEP_WALK + 2) {
        g.blk(2, y, NOOK_Z0 - 1, pick(&keep_wall(), 0.3), None);
        g.blk(2, y, NOOK_Z1 + 1, pick(&keep_wall(), 0.6), None);
    }

    // ---- 4. Dressing: braziers, a rack, sea-worn tufts ----------------------
    for z in [6, 14, 22, 30] {
        g.blk(
            LANE_X1,
            KEEP_WALK,
            z,
            "minecraft:lantern",
            Some(vec![("hanging", "false")]),
        );
    }
    for z in [9, 19, 29] {
        g.blk(
            LANE_X0,
            KEEP_WALK,
            z,
            "minecraft:campfire",
            Some(vec![("lit", "true"), ("facing", "north")]),
        );
    }
    for x in LANE_X0..=LANE_X1 {
        for z in 0..SZ {
            if g.is_air(x, KEEP_WALK, z) && hash01(seed, x, KEEP_WALK, z, 23) < 0.05 {
                g.blk(x, KEEP_WALK, z, "minecraft:dead_bush", None);
            }
        }
    }

    // ---- 5. Sockets ---------------------------------------------------------
    cut_socket(g, Side::South, KEEP_FLOOR_Y, 7);
    cut_socket(g, Side::North, KEEP_FLOOR_Y, 7);

    // ---- 6. Invariant: the parapet run walks end to end --------------------
    let route: Vec<[i32; 3]> = (0..SZ).rev().map(|z| [7, KEEP_WALK, z]).collect();
    assert_route_walkable("tk-wall-walk", "parapet run", g, &route);
}

pub fn anchors() -> Vec<(&'static str, AnchorJson)> {
    let y = KEEP_WALK;
    vec![
        ("anchor/l1b-parapet-south", a_pos([7, y, 30], "north")),
        ("anchor/l1b-ambush-door", a_pos([1, y, 16], "east")),
        ("anchor/l1b-parapet-mid", a_pos([7, y, 16], "north")),
        ("anchor/l1b-breach-view", a_pos([7, y, 25], "west")),
        ("anchor/l1b-parapet-north", a_pos([7, y, 4], "north")),
        ("anchor/l1b-lane-1", a_pos([7, y, 28], "north")),
        ("anchor/l1b-lane-2", a_pos([7, y, 16], "north")),
        ("anchor/l1b-lane-3", a_pos([7, y, 4], "north")),
    ]
}
