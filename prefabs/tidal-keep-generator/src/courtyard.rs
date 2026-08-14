//! `tk-courtyard-chapel` (L2) — the hub.
//!
//! One large open muster yard with **two physically distinct breach lanes** (a
//! collapsed section of the south curtain and one of the west curtain, each with
//! its own rubble ramp and its own waypoint chain) converging on the muster
//! anchor: the TD siege of spec-0016 §6 needs lanes that are legible from the
//! ground, not just waypoints in a JSON file.
//!
//! The chapel occupies the east range so its flank wall IS the piece's east face
//! — the undercroft socket opens straight out of it, which is what makes "the
//! stair down is in the chapel" true in geometry rather than in prose. Inside:
//! the hearth (BF2, the regroup/dialogue stage), the cracked bell on its frame,
//! and the altar.

use crate::common::*;

pub const SX: i32 = 46;
pub const SY: i32 = 26;
pub const SZ: i32 = 46;

/// Curtain-wall thickness.
const W: i32 = 3;
const WALL_TOP: i32 = 17;
/// Chapel footprint (its east wall is the piece's east face).
const CH_X0: i32 = 28;
const CH_Z0: i32 = 10;
const CH_Z1: i32 = 28;
const CH_CEIL: i32 = 19;
/// Muster point the two lanes converge on.
const MUSTER: [i32; 3] = [23, KEEP_WALK, 23];

pub fn build(g: &mut Grid, seed: u64) {
    // ---- 1. Plinth + curtain walls -----------------------------------------
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
            let perim = !(W..SX - W).contains(&x) || !(W..SZ - W).contains(&z);
            if perim {
                for y in KEEP_WALK..=WALL_TOP {
                    g.blk(
                        x,
                        y,
                        z,
                        pick(&keep_wall(), value_noise(seed, x, y, z, 0.16, 15)),
                        None,
                    );
                }
                // crenellated wall-head
                if (x == W - 1 || x == SX - W || z == W - 1 || z == SZ - W) && (x + z) % 2 == 0 {
                    g.blk(x, WALL_TOP + 1, z, "minecraft:stone_brick_wall", None);
                }
            }
        }
    }
    // yard surface
    g.fill_pal(
        bx(W, SX - W - 1, KEEP_FLOOR_Y, KEEP_FLOOR_Y, W, SZ - W - 1),
        &keep_floor(),
        seed,
        0.18,
        17,
    );

    // ---- 2. The two breaches -----------------------------------------------
    // GATE BREACH — the south curtain, west of the gate socket.
    g.carve(bx(8, 14, KEEP_WALK, WALL_TOP, SZ - W, SZ - 1));
    breach_rubble(
        g,
        seed,
        bx(8, 14, KEEP_WALK, KEEP_WALK + 2, SZ - W - 3, SZ - W - 1),
        31,
    );
    // WALL BREACH — the west curtain.
    g.carve(bx(0, W - 1, KEEP_WALK, WALL_TOP, 14, 20));
    breach_rubble(g, seed, bx(W, W + 2, KEEP_WALK, KEEP_WALK + 2, 14, 20), 33);

    // ---- 3. The chapel ------------------------------------------------------
    for x in CH_X0..SX {
        for z in CH_Z0..=CH_Z1 {
            for y in KEEP_WALK..=CH_CEIL {
                g.blk(
                    x,
                    y,
                    z,
                    pick(&keep_wall(), value_noise(seed, x, y, z, 0.14, 19)),
                    None,
                );
            }
        }
    }
    // interior
    g.carve(bx(
        CH_X0 + 2,
        SX - W - 1,
        KEEP_WALK,
        CH_CEIL - 1,
        CH_Z0 + 2,
        CH_Z1 - 2,
    ));
    g.fill_pal(
        bx(
            CH_X0 + 2,
            SX - W - 1,
            KEEP_FLOOR_Y,
            KEEP_FLOOR_Y,
            CH_Z0 + 2,
            CH_Z1 - 2,
        ),
        &keep_floor(),
        seed,
        0.22,
        21,
    );
    // west door onto the yard (3 wide)
    g.carve(bx(CH_X0, CH_X0 + 1, KEEP_WALK, KEEP_WALK + 2, 18, 20));
    // clerestory slits so the nave reads as a chapel, not a bunker
    for z in (CH_Z0 + 4..=CH_Z1 - 4).step_by(4) {
        g.carve(bx(CH_X0, CH_X0 + 1, KEEP_WALK + 4, KEEP_WALK + 5, z, z));
    }
    // gabled roof
    for k in 0..4 {
        g.fill_pal(
            bx(
                CH_X0 + k,
                SX - 1 - k,
                CH_CEIL + k,
                CH_CEIL + k,
                CH_Z0 + k,
                CH_Z1 - k,
            ),
            &keep_wall(),
            seed,
            0.3,
            23,
        );
    }

    // ---- 4. Chapel fixtures: hearth (BF2), cracked bell, altar --------------
    // Hearth in the SOUTH wall, at the far (east) end of the nave, beside the
    // undercroft door — the last fire before the drowned way down.
    //
    // Where it may stand is a `DW0478` constraint, not composition: the
    // gate-breach siege lane (`aggro_radius` 16) ends
    // at `anchor/l2-lane-gate-3` (16,_,19) out in the yard, and a marching squad
    // is a corridor around its polyline — the measured 7.9-block drift — so the
    // fire must clear the lane by more than 23.9 blocks. The hearth's first home
    // in the north wall at (33,_,12) put its rest cell 18.0 blocks out, inside
    // the marching squad's reach. The chapel's east corners are the only cells
    // that clear it: the rest cell now sits at (41,_,25), 25.7 blocks from the
    // lane end. That is the most this room can give — no interior cell exceeds
    // ~27 blocks — so the margin is 1.8 blocks and comes from geometry, not from
    // taste. It is also the better fire: the party regroups at the head of the
    // stair they are about to go down, not in the corner the breach opens onto.
    g.carve(bx(40, 42, KEEP_WALK, KEEP_WALK + 3, CH_Z1 - 2, CH_Z1 - 1));
    g.blk(
        41,
        KEEP_WALK,
        CH_Z1 - 2,
        "minecraft:campfire",
        Some(vec![("lit", "true"), ("facing", "north")]),
    );
    for x in [40, 42] {
        for y in KEEP_WALK..=(KEEP_WALK + 3) {
            g.blk(x, y, CH_Z1 - 2, "minecraft:mossy_cobblestone", None);
        }
    }
    // the cracked bell: a bell hung in a stone-and-timber frame mid-nave
    for x in [36, 38] {
        for y in KEEP_WALK..=(KEEP_WALK + 3) {
            g.blk(x, y, 20, "minecraft:stone_bricks", None);
        }
    }
    g.blk(
        37,
        KEEP_WALK + 3,
        20,
        "minecraft:dark_oak_log",
        Some(vec![("axis", "x")]),
    );
    g.blk(
        37,
        KEEP_WALK + 2,
        20,
        "minecraft:bell",
        Some(vec![("attachment", "ceiling"), ("facing", "north")]),
    );
    // the crack: a spill of shattered masonry under the bell
    for (dx, dz) in [(-1, 1), (0, 1), (1, 1), (0, 2)] {
        g.blk(
            37 + dx,
            KEEP_WALK,
            20 + dz,
            "minecraft:cracked_stone_bricks",
            None,
        );
    }
    // altar at the east end, beside the undercroft door
    g.blk(41, KEEP_WALK, 15, "minecraft:chiseled_stone_bricks", None);
    g.blk(
        41,
        KEEP_WALK + 1,
        15,
        "minecraft:stone_brick_slab",
        Some(vec![("type", "bottom")]),
    );
    // chapel lighting
    for z in (CH_Z0 + 3..=CH_Z1 - 3).step_by(4) {
        g.blk(
            CH_X0 + 2,
            KEEP_WALK + 3,
            z,
            "minecraft:lantern",
            Some(vec![("hanging", "false")]),
        );
        g.blk(
            SX - W - 1,
            KEEP_WALK + 3,
            z,
            "minecraft:lantern",
            Some(vec![("hanging", "false")]),
        );
    }

    light_room(
        g,
        CH_X0 + 2,
        SX - W - 1,
        CH_Z0 + 2,
        CH_Z1 - 2,
        KEEP_WALK,
        CH_CEIL,
        5,
        "minecraft:lantern",
    );

    // ---- 5. Yard dressing ---------------------------------------------------
    // a muster ring of standing posts around the defended anchor
    for (dx, dz) in [(-5, -5), (5, -5), (-5, 5), (5, 5)] {
        let (px, pz) = (MUSTER[0] + dx, MUSTER[2] + dz);
        if on_lane(px, pz) {
            continue;
        }
        for y in KEEP_WALK..=(KEEP_WALK + 2) {
            g.blk(px, y, pz, "minecraft:mossy_stone_bricks", None);
        }
        g.blk(
            px,
            KEEP_WALK + 3,
            pz,
            "minecraft:lantern",
            Some(vec![("hanging", "false")]),
        );
    }
    // a well (sealed — the cistern below is reached by the chapel stair, not here),
    // set well clear of both lanes
    for (wx, wz) in [(6, 33), (8, 33), (6, 35), (8, 35)] {
        g.blk(wx, KEEP_WALK, wz, "minecraft:mossy_cobblestone", None);
        g.blk(wx, KEEP_WALK + 1, wz, "minecraft:cobblestone_wall", None);
    }
    g.fill(
        bx(6, 8, KEEP_WALK, KEEP_WALK, 34, 34),
        "minecraft:cobblestone",
        None,
    );
    g.fill(
        bx(7, 7, KEEP_WALK, KEEP_WALK, 33, 35),
        "minecraft:cobblestone",
        None,
    );
    // scattered siege spoil, never on a lane
    for x in W..SX - W {
        for z in W..SZ - W {
            if !g.is_air(x, KEEP_WALK, z) || on_lane(x, z) || near_anchor(&anchors(), x, z, 2) {
                continue;
            }
            let n = hash01(seed, x, KEEP_WALK, z, 41);
            if n < 0.03 {
                g.blk(x, KEEP_WALK, z, "minecraft:cobblestone_wall", None);
            } else if n < 0.07 {
                g.blk(x, KEEP_WALK, z, "minecraft:dead_bush", None);
            }
        }
    }

    // ---- 6. Sockets ---------------------------------------------------------
    cut_socket(g, Side::South, KEEP_FLOOR_Y, 23);
    g.carve(bx(22, 24, KEEP_WALK, KEEP_WALK + 2, SZ - W, SZ - 1));
    cut_socket(g, Side::East, KEEP_FLOOR_Y, 23);
    g.carve(bx(SX - W, SX - 1, KEEP_WALK, KEEP_WALK + 2, 22, 24));

    // ---- 7. Invariants: both lanes and both breaches walk ------------------
    for (label, chain) in lane_chains() {
        let route = trace(&chain);
        assert_route_walkable("tk-courtyard-chapel", label, g, &route);
    }
    let chapel = trace(&[
        [24, KEEP_WALK, 19],
        [CH_X0 + 3, KEEP_WALK, 19],
        [40, KEEP_WALK, 15],
    ]);
    assert_route_walkable(
        "tk-courtyard-chapel",
        "yard -> chapel -> undercroft door",
        g,
        &chapel,
    );
}

fn breach_rubble(g: &mut Grid, seed: u64, b: [i32; 6], salt: u64) {
    for x in b[0]..=b[1] {
        for z in b[4]..=b[5] {
            let h = (value_noise(seed, x, 0, z, 0.5, salt) * 2.4) as i32;
            for y in b[2]..(b[2] + h) {
                g.blk(x, y, z, "minecraft:cobblestone", None);
            }
        }
    }
    // a rubble ramp is only scenery if it is not walkable; keep it low and leave
    // the lane cells themselves clear (the lane invariant re-proves this).
    for x in b[0]..=b[1] {
        for z in b[4]..=b[5] {
            if on_lane(x, z) {
                g.carve(bx(x, x, b[2], b[3], z, z));
                g.blk(x, b[2] - 1, z, "minecraft:cobblestone", None);
            }
        }
    }
}

/// The two lane spines (breach → muster), as waypoint chains. Consecutive lane
/// waypoints are > 10 blocks apart (spec-0016 §6: the vanilla patrol goal
/// re-rolls its target inside 10).
fn lane_chains() -> Vec<(&'static str, Vec<[i32; 3]>)> {
    let y = KEEP_WALK;
    vec![
        (
            "gate-breach lane",
            vec![[11, y, 41], [11, y, 29], [16, y, 19], MUSTER],
        ),
        (
            "wall-breach lane",
            vec![[4, y, 17], [16, y, 17], [24, y, 11], MUSTER],
        ),
    ]
}

/// Whether (x,z) lies on either lane spine (3 wide), so dressing keeps off it.
fn on_lane(x: i32, z: i32) -> bool {
    for (_, chain) in lane_chains() {
        for w in chain.windows(2) {
            for c in seg(w[0], w[1]) {
                if (c[0] - x).abs() <= 1 && (c[2] - z).abs() <= 1 {
                    return true;
                }
            }
        }
    }
    false
}

/// Cardinal (L-shaped) trace between two same-y cells: x first, then z.
fn seg(a: [i32; 3], b: [i32; 3]) -> Vec<[i32; 3]> {
    let mut v = vec![a];
    let mut c = a;
    while c[0] != b[0] {
        c[0] += (b[0] - c[0]).signum();
        v.push(c);
    }
    while c[2] != b[2] {
        c[2] += (b[2] - c[2]).signum();
        v.push(c);
    }
    v
}

fn trace(chain: &[[i32; 3]]) -> Vec<[i32; 3]> {
    let mut out: Vec<[i32; 3]> = Vec::new();
    for w in chain.windows(2) {
        for c in seg(w[0], w[1]) {
            if out.last() != Some(&c) {
                out.push(c);
            }
        }
    }
    out
}

pub fn anchors() -> Vec<(&'static str, AnchorJson)> {
    let y = KEEP_WALK;
    vec![
        ("anchor/l2-gate-door", a_pos([23, y, 41], "south")),
        ("anchor/l2-muster", a_pos(MUSTER, "south")),
        // 25.7 blocks from the gate lane's end — the `DW0478` clearance.
        ("anchor/l2-bonfire", a_pos([41, y, 25], "north")),
        ("anchor/l2-cracked-bell", a_pos([37, y, 23], "north")),
        ("anchor/l2-altar", a_pos([40, y, 15], "east")),
        ("anchor/l2-chapel-door", a_pos([27, y, 19], "east")),
        ("anchor/l2-undercroft-door", a_pos([42, y, 23], "east")),
        ("anchor/l2-tower-view", a_pos([23, y, 6], "north")),
        ("anchor/l2-well", a_pos([7, y, 31], "south")),
        (
            "anchor/l2-breach-gate",
            a_region([8, y, SZ - 2], [14, y + 4, SZ - 1], "minecraft:cobblestone"),
        ),
        (
            "anchor/l2-breach-wall",
            a_region([0, y, 14], [1, y + 4, 20], "minecraft:cobblestone"),
        ),
        ("anchor/l2-lane-gate-1", a_pos([11, y, 41], "north")),
        ("anchor/l2-lane-gate-2", a_pos([11, y, 29], "north")),
        ("anchor/l2-lane-gate-3", a_pos([16, y, 19], "east")),
        ("anchor/l2-lane-wall-1", a_pos([4, y, 17], "east")),
        ("anchor/l2-lane-wall-2", a_pos([16, y, 17], "east")),
        ("anchor/l2-lane-wall-3", a_pos([24, y, 11], "south")),
    ]
}

pub fn light_regions() -> Vec<[i32; 6]> {
    vec![bx(
        CH_X0 + 2,
        SX - W - 1,
        KEEP_WALK,
        KEEP_WALK + 1,
        CH_Z0 + 2,
        CH_Z1 - 2,
    )]
}
