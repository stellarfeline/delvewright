//! `tk-barrow-field` (L0) — the shore the delve opens on.
//!
//! A terrain-scale beach landing rising into an open barrow field, built on the
//! shared shore datum (`prefabs/island-tileset.md`: waterline local y=2, walk
//! plane local y=3) so the authored sea meets a `horizon: ocean` world at sea
//! level (`DW0344`) and the strand is climb-out-able from the water (`DW0322`).
//!
//! The whole piece exists to stage one beat: the **optional elite** standing
//! athwart the spawn→gate desire line. Its legibility (souls dossier §3.3) is
//! GEOMETRY, not signage — so the field is deliberately WIDE and empty on both
//! flanks: the burial mounds are set back off two open flank lanes, and the
//! generator proves all three routes (centre + both flanks) walkable before the
//! piece is written. If a mound ever crept into a flank the bypass would become
//! a lie, and "optional" with it.

use crate::common::*;

pub const SX: i32 = 48;
pub const SY: i32 = 14;
pub const SZ: i32 = 40;

/// Centre x of the spawn→gate desire line.
const LINE_X: i32 = 24;
/// The two open flank lanes the elite must be walk-past-able on.
const FLANK_W: i32 = 7;
const FLANK_E: i32 = 40;
/// First land row north of the tide.
const TIDE_Z: i32 = 33;

/// Burial mounds: `(cx, cz, radius)`. Every one is checked against the corridor
/// and both flank lanes by `assert_field_open`.
const BARROWS: [(i32, i32, i32); 4] = [(14, 10, 4), (33, 12, 4), (19, 16, 2), (35, 24, 3)];

/// Top solid y of the ground at (x,z) before the mounds are raised. `None` means
/// open sea: seabed solid at y=0, water at y=1..2.
fn surface(x: i32, z: i32) -> Option<i32> {
    if z > TIDE_Z {
        return None;
    }
    // Coastal bluffs bound the box-garden so no walkable cell borders the void.
    let edge_x = x.min(SX - 1 - x);
    if edge_x <= 1 {
        return Some(SHORE_FLOOR_Y + 4 - edge_x);
    }
    if z <= 1 {
        // the north bluff is the keep's outwork; it dips only in the socket lane
        if (x - LINE_X).abs() <= 2 {
            return Some(SHORE_FLOOR_Y);
        }
        return Some(SHORE_FLOOR_Y + 5 - z);
    }
    Some(SHORE_FLOOR_Y)
}

fn mound_height(x: i32, z: i32) -> i32 {
    let mut h = 0;
    for (cx, cz, rad) in BARROWS {
        let (dx, dz) = ((x - cx) as f64 / rad as f64, (z - cz) as f64 / rad as f64);
        let r = (dx * dx + dz * dz).sqrt();
        if r < 1.0 {
            h = h.max(((1.0 - r) * (rad as f64 * 0.85)).round() as i32);
        }
    }
    h
}

/// The three routes this piece promises: the centre desire line and the two
/// flanks that make the elite optional. One definition, shared by the dressing
/// filter and the walkability proof, so they cannot drift apart.
fn routes() -> Vec<(&'static str, Vec<[i32; 3]>)> {
    let y = SHORE_WALK;
    let leg = |from: [i32; 3], to: [i32; 3], v: &mut Vec<[i32; 3]>| {
        let mut c = from;
        if v.last() != Some(&c) {
            v.push(c);
        }
        while c[0] != to[0] {
            c[0] += (to[0] - c[0]).signum();
            v.push(c);
        }
        while c[2] != to[2] {
            c[2] += (to[2] - c[2]).signum();
            v.push(c);
        }
    };
    let mut centre = Vec::new();
    leg([LINE_X, y, 31], [LINE_X, y, 3], &mut centre);
    let flank = |fx: i32, name: &'static str| -> (&'static str, Vec<[i32; 3]>) {
        let mut v = Vec::new();
        leg([LINE_X, y, 31], [LINE_X, y, 32], &mut v);
        leg([LINE_X, y, 32], [fx, y, 32], &mut v);
        leg([fx, y, 32], [fx, y, 4], &mut v);
        leg([fx, y, 4], [LINE_X, y, 4], &mut v);
        (name, v)
    };
    vec![
        ("centre desire line", centre),
        flank(FLANK_W, "west flank (elite bypass)"),
        flank(FLANK_E, "east flank (elite bypass)"),
    ]
}

/// Whether (x,z) sits on (or beside) any promised route, so dressing keeps off.
fn on_route(x: i32, z: i32) -> bool {
    routes().iter().any(|(_, r)| {
        r.iter()
            .any(|c| (c[0] - x).abs() <= 1 && (c[2] - z).abs() <= 1)
    })
}

pub fn build(g: &mut Grid, seed: u64) {
    // 1. Ground: solid plinth up to the surface, sand at the tide, turf inland.
    for x in 0..SX {
        for z in 0..SZ {
            match surface(x, z) {
                None => {
                    g.blk(
                        x,
                        0,
                        z,
                        // seabed: the piece's bottom layer, so it must carry NO
                        // gravity block (nothing supports y=0 inside the piece)
                        pick(&plinth(), value_noise(seed, x, 0, z, 0.2, 11)),
                        None,
                    );
                    g.blk(x, 1, z, "minecraft:water", None);
                    g.blk(x, 2, z, "minecraft:water", None);
                }
                Some(top) => {
                    let top = top + mound_height(x, z);
                    for y in 0..=top {
                        let name = if y == top {
                            if top <= SHORE_FLOOR_Y && z >= TIDE_Z - 3 {
                                pick(&shore_sand(), value_noise(seed, x, y, z, 0.18, 13))
                            } else if mound_height(x, z) > 0 {
                                pick(&barrow_stone(), value_noise(seed, x, y, z, 0.3, 15))
                            } else if top > SHORE_FLOOR_Y {
                                pick(&plinth(), value_noise(seed, x, y, z, 0.16, 17))
                            } else {
                                pick(&turf(), value_noise(seed, x, y, z, 0.15, 19))
                            }
                        } else if y >= SHORE_FLOOR_Y {
                            pick(&plinth(), value_noise(seed, x, y, z, 0.16, 21))
                        } else {
                            "minecraft:stone"
                        };
                        g.blk(x, y, z, name, None);
                    }
                }
            }
        }
    }

    // 2. Barrow portals: a lintelled slot on the south face of each mound, and a
    //    kerb of standing stones around it (lore props, no interiors — the
    //    mounds are sealed, so nothing here can strand a player).
    for (cx, cz, rad) in BARROWS {
        let pz = cz + rad;
        for dx in -1..=1 {
            for y in SHORE_WALK..=(SHORE_WALK + 1) {
                g.blk(cx + dx, y, pz, "minecraft:mossy_cobblestone", None);
            }
        }
        g.blk(cx, SHORE_WALK, pz, "minecraft:cobblestone_wall", None);
        for (kx, kz) in [(-rad - 1, 0), (rad + 1, 0), (0, -rad - 1)] {
            g.blk(
                cx + kx,
                SHORE_WALK,
                cz + kz,
                "minecraft:cobblestone_wall",
                None,
            );
            g.blk(
                cx + kx,
                SHORE_WALK + 1,
                cz + kz,
                "minecraft:cobblestone_wall",
                None,
            );
        }
    }

    // 3. Shore dressing: driftwood, seagrass in the shallows, dead bushes on the
    //    dunes, tufts on the field. Never on a route cell (proved in step 6).
    for x in 2..SX - 2 {
        for z in 2..SZ {
            if surface(x, z).is_none() {
                if value_noise(seed, x, 1, z, 0.4, 31) > 0.80 {
                    g.blk(x, 1, z, "minecraft:seagrass", None);
                }
                continue;
            }
            if !g.is_air(x, SHORE_WALK, z) || mound_height(x, z) > 0 {
                continue;
            }
            if on_route(x, z) || near_anchor(&anchors(), x, z, 2) {
                continue;
            }
            let n = hash01(seed, x, SHORE_WALK, z, 33);
            if z >= TIDE_Z - 3 {
                if n < 0.05 {
                    g.blk(x, SHORE_WALK, z, "minecraft:dead_bush", None);
                }
            } else if n < 0.10 {
                g.blk(x, SHORE_WALK, z, "minecraft:short_grass", None);
            }
        }
    }
    // driftwood spars above the tide line (two fixed, deterministic). The west
    // spar sits between the landing and the fire; it was moved off x=11 when BF1
    // moved west (below), because a spar log on the rest cell is a spar log the
    // anchor cannot stand on.
    for (dx, dz) in [(15, 30), (34, 30)] {
        for k in 0..3 {
            g.blk(
                dx + k,
                SHORE_WALK,
                dz,
                "minecraft:oak_log",
                Some(vec![("axis", "x")]),
            );
        }
    }

    // 4. BF1 "Barrow Fire": a lit campfire in a stone ring, its rest cell SOUTH
    //    of the flame so the anchor stays standable (`DW0316`) and the party
    //    rests with the field — and the kneeling elite — in front of them.
    //
    //    Placement is a `DW0478` constraint, not taste: a bonfire may not stand
    //    inside any hostile's aggro range,
    //    and the Barrow Warden kneels at `anchor/l0-elite-dormant` (23,_,16) with
    //    the default 16-block `follow_range`. The fire's first home at (19,_,29)
    //    was 13.6 blocks out — inside its sight. It now sits far down the western
    //    strand at (10,_,31) with its rest cell at (10,_,30+1), 19.8 blocks from
    //    the kneel: still on the landing beach, still visible from spawn across
    //    open sand, still the first thing the shore offers — and out of the
    //    warden's reach with margin. It cannot go straight south instead: the
    //    tide bounds the piece at z=33, so the whole shore is at most 17 blocks
    //    from the kneel on the centre line, and clearing 16 needs the lateral
    //    run. Every cell here is off all three proved routes (the centre desire
    //    line at x=24, both flank lanes at x=7/x=40 and the z=32 traverse).
    for (fx, fz) in [(11, 29), (10, 29), (9, 29), (11, 30), (9, 30), (11, 31)] {
        g.blk(fx, SHORE_WALK, fz, "minecraft:cobblestone", None);
    }
    g.blk(
        10,
        SHORE_WALK,
        30,
        "minecraft:campfire",
        Some(vec![("lit", "true"), ("facing", "south")]),
    );
    g.blk(
        10,
        SHORE_WALK,
        28,
        "minecraft:oak_log",
        Some(vec![("axis", "x")]),
    );

    // 5. The elite's dressing: a leaning banner pole beside the barrow it sleeps
    //    against — the conspicuous over-dress that reads "this one is different"
    //    from the far end of the field (dossier §3.3 signal 4).
    for y in SHORE_WALK..=(SHORE_WALK + 3) {
        g.blk(23, y, 14, "minecraft:oak_fence", None);
    }
    g.blk(23, SHORE_WALK + 4, 14, "minecraft:black_wool", None);
    g.blk(23, SHORE_WALK + 3, 15, "minecraft:black_wool", None);
    // the reward cache behind the elite: a barrel on a kerb, west field
    g.blk(12, SHORE_WALK, 21, "minecraft:cobblestone", None);
    g.blk(
        12,
        SHORE_WALK + 1,
        21,
        "minecraft:barrel",
        Some(vec![("facing", "up")]),
    );

    // 6. Sockets: the gate approach (north, shore datum).
    cut_socket(g, Side::North, SHORE_FLOOR_Y, LINE_X);

    // 7. Invariants: the field is open on both flanks, and every promised route
    //    walks under the current nav model.
    assert_field_open();
    for (label, route) in routes() {
        assert_route_walkable("tk-barrow-field", label, g, &route);
    }
}

/// The bypass promise, proved at generation time: no burial mound may reach the
/// centre corridor or either flank lane. An "optional" elite whose flank ground
/// is blocked is not optional — and the compiler's optional-elite bypass proof
/// would be arguing about geometry this piece got wrong.
fn assert_field_open() {
    for (cx, cz, rad) in BARROWS {
        for lane in [LINE_X - 1, LINE_X, LINE_X + 1, FLANK_W, FLANK_E] {
            let clear = (cx - lane).abs() > rad + 1;
            assert!(
                clear,
                "tk-barrow-field: barrow at ({cx},{cz}) r{rad} intrudes on lane x={lane} — the \
                 elite's flank ground must stay open or the bypass is a lie"
            );
        }
    }
}

pub fn anchors() -> Vec<(&'static str, AnchorJson)> {
    let y = SHORE_WALK;
    vec![
        ("spawn", a_pos([LINE_X, y, 31], "north")),
        // 19.8 blocks from `anchor/l0-elite-dormant` — the `DW0478` clearance.
        ("anchor/l0-bonfire", a_pos([10, y, 31], "north")),
        ("anchor/l0-tide-line", a_pos([LINE_X, y, 33], "south")),
        ("anchor/l0-elite-stand", a_pos([LINE_X, y, 18], "south")),
        ("anchor/l0-elite-dormant", a_pos([23, y, 16], "south")),
        ("anchor/l0-banner", a_pos([22, y, 14], "east")),
        ("anchor/l0-flank-west", a_pos([FLANK_W, y, 18], "north")),
        ("anchor/l0-flank-east", a_pos([FLANK_E, y, 18], "north")),
        ("anchor/l0-barrow-1", a_pos([14, y, 15], "north")),
        ("anchor/l0-barrow-2", a_pos([33, y, 17], "north")),
        ("anchor/l0-barrow-3", a_pos([19, y, 19], "north")),
        ("anchor/l0-barrow-4", a_pos([35, y, 28], "north")),
        ("anchor/l0-reward", a_pos([12, y, 20], "north")),
        // The barrel `anchor/l0-reward` stands in front of (spec-0021 `loot[]`
        // names the container CELL, not the footing beside it).
        (
            "anchor/l0-reward-cache",
            a_container([12, y + 1, 21], "north"),
        ),
        ("anchor/l0-gate-approach", a_pos([LINE_X, y, 4], "north")),
    ]
}
