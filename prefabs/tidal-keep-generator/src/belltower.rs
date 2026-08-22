//! `tk-bell-tower` (L4) — rope room, bell loft, boss ring, and the drop home.
//!
//! Three stacked rooms around one vertical void:
//!
//! * **rope room** (walk 11) — BF3, deliberately BEFORE the fog line (the DS3
//!   correction): the runback from here to the ring is two stair flights.
//!   The bell pit's water basin sits in its floor.
//! * **bell loft** (walk 23) — four rafter perches at eye-catching height, every
//!   one of them in clean line of sight from the loft doorway. The TWIST ambush
//!   is meant to be *seen* and beaten: no telegraph, only sightline (dossier
//!   §4.3). The generator proves the sightlines rather than trusting the layout.
//! * **boss ring** (walk 33) — an open annulus around the pit, with pillar cover
//!   and a raised outer walk. Anti-Capra by construction: no closets, no
//!   chokepoint, and the keeper is visible from the ring doorway.
//!
//! The **bell-rope drop** is the hub-opener. It is one-way BY GEOMETRY, not by
//! script: the pit falls twenty-two blocks into a three-deep water basin beside
//! the rope room's socket, and its walls are sheer. Nothing climbable is used —
//! the compiler's nav model does not model ladders or vines at all, so a climb
//! route would be an unprovable promise. The stairs remain walkable both ways, so
//! the model always has a proven return and nothing can strand (`DW0315`).
//!
//! The **tide gate + ferry pier**: the rope room's
//! south wall carries a sealed sea-door (`anchor/l4-tide-gate`, iron bars — the
//! sea is visible through it from BF3, and no lever anywhere opens it) giving
//! onto a walled sea-stair down the tower's south face to a stone ferry pier at
//! the shore datum. The campaign's finale opens the gate; the road home after
//! the boss is then the tower's own interior and this stair — it never re-enters
//! the courtyard. The pier is authored sea, so the piece declares `waterline_y`
//! and lands on the same ocean datum as the barrow shore (`DW0344`).

use crate::common::*;

pub const SX: i32 = 26;
pub const SY: i32 = 44;
/// Tower body depth. The piece is deeper (`SZ`): local z `TZ..` is the authored
/// sea band the ferry pier stands in.
pub const TZ: i32 = 26;
pub const SZ: i32 = 36;

const IN0: i32 = 2;
const IN1: i32 = 23;
/// Room data: (floor top, walk, ceiling air top).
const ANTE_FLOOR: i32 = KEEP_FLOOR_Y;
const ANTE_CEIL: i32 = 17;
const LOFT_FLOOR: i32 = 22;
const LOFT_WALK: i32 = 23;
const LOFT_CEIL: i32 = 31;
const RING_FLOOR: i32 = 32;
const RING_WALK: i32 = 33;
const RING_CEIL: i32 = 40;
/// The bell pit / rope shaft.
const PIT0: i32 = 11;
const PIT1: i32 = 14;
/// Rafter beam height and the perch plane above it.
const RAFTER: i32 = 27;
/// Stairwells.
const S1_X0: i32 = 3;
const S1_X1: i32 = 5;
const S2_X0: i32 = 20;
const S2_X1: i32 = 22;
/// The tide gate: a 3-wide, 3-tall doorway through the rope room's south wall
/// (z `TZ-2..TZ-1`), sealed with iron bars from world-load. The finale's
/// `open-gate` clears exactly this region.
const TG_X0: i32 = 12;
const TG_X1: i32 = 14;
/// The sea-stair outside it: one tread per z, dropping from the keep walk to the
/// shore walk, parapet-walled on both flanks.
const SEA_Z0: i32 = TZ;
const SEA_Z1: i32 = 32;
/// The ferry pier deck (walk = `SHORE_WALK`, the barrow-shore datum).
const PIER_X0: i32 = 9;
const PIER_X1: i32 = 17;
const PIER_Z0: i32 = 33;

fn sea_stair_walk(z: i32) -> i32 {
    KEEP_WALK - 1 - (z - SEA_Z0)
}

fn stair1_walk(z: i32) -> i32 {
    KEEP_WALK + (15 - z)
}
fn stair2_walk(z: i32) -> i32 {
    LOFT_WALK + (13 - z)
}

/// The four rafter perches (the TWIST ambush). Kept as one list so the metadata,
/// the geometry and the sightline proof cannot drift apart.
const PERCHES: [[i32; 3]; 4] = [
    [6, RAFTER + 1, 6],
    [19, RAFTER + 1, 6],
    [6, RAFTER + 1, 18],
    [19, RAFTER + 1, 18],
];
/// Where the loft is entered from — every perch must be visible from here.
const LOFT_DOOR: [i32; 3] = [4, LOFT_WALK, 3];

pub fn build(g: &mut Grid, seed: u64) {
    // ---- 1. Solid tower (body only: the sea band `TZ..SZ` stays open) ------
    for x in 0..SX {
        for z in 0..TZ {
            for y in 0..SY {
                let name = if y < SHORE_FLOOR_Y + 4 {
                    pick(&plinth(), value_noise(seed, x, y, z, 0.14, 11))
                } else {
                    pick(&keep_wall(), value_noise(seed, x, y, z, 0.11, 13))
                };
                g.blk(x, y, z, name, None);
            }
        }
    }

    // ---- 2. The three rooms -------------------------------------------------
    for (floor, ceil, salt) in [
        (ANTE_FLOOR, ANTE_CEIL, 15u64),
        (LOFT_FLOOR, LOFT_CEIL, 17),
        (RING_FLOOR, RING_CEIL, 19),
    ] {
        g.carve(bx(IN0, IN1, floor + 1, ceil, IN0, IN1));
        g.fill_pal(
            bx(IN0, IN1, floor, floor, IN0, IN1),
            &keep_floor(),
            seed,
            0.2,
            salt,
        );
    }

    // ---- 3. The bell pit: one sheer void from the ring to the water --------
    g.carve(bx(PIT0, PIT1, 8, RING_CEIL, PIT0, PIT1));
    g.fill_pal(
        bx(PIT0, PIT1, 7, 7, PIT0, PIT1),
        &keep_floor(),
        seed,
        0.3,
        21,
    );
    g.fill(
        bx(PIT0, PIT1, 8, ANTE_FLOOR, PIT0, PIT1),
        "minecraft:water",
        None,
    );
    // a kerb at the rope-room floor so the basin lip reads, and a wall course on
    // both upper floors so nobody walks into the void by accident
    for x in (PIT0 - 1)..=(PIT1 + 1) {
        for z in (PIT0 - 1)..=(PIT1 + 1) {
            let lip = x == PIT0 - 1 || x == PIT1 + 1 || z == PIT0 - 1 || z == PIT1 + 1;
            if !lip {
                continue;
            }
            g.blk(x, KEEP_WALK, z, "minecraft:stone_brick_wall", None);
            g.blk(x, LOFT_WALK, z, "minecraft:stone_brick_wall", None);
        }
    }
    // the rope-drop grate at ring level: the campaign's hub-opener clears it
    g.fill(
        bx(PIT0, PIT1, RING_WALK, RING_WALK, PIT0, PIT1),
        "minecraft:iron_bars",
        None,
    );
    // the bell itself, hung high over the pit on a dark-oak headstock
    g.fill(
        bx(PIT0, PIT1, RING_CEIL - 1, RING_CEIL - 1, 12, 13),
        "minecraft:dark_oak_log",
        Some(vec![("axis", "x")]),
    );
    g.blk(
        12,
        RING_CEIL - 2,
        12,
        "minecraft:bell",
        Some(vec![("attachment", "ceiling"), ("facing", "north")]),
    );

    // ---- 4. Rope room: BF3, the basin lip, the socket ----------------------
    g.blk(
        8,
        KEEP_WALK,
        18,
        "minecraft:campfire",
        Some(vec![("lit", "true"), ("facing", "north")]),
    );
    for (cx, cz) in [(7, 18), (9, 18), (8, 17), (8, 19)] {
        g.blk(cx, KEEP_WALK, cz, "minecraft:cobblestone", None);
    }
    for (lx, lz) in [(IN0, 6), (IN0, 20), (IN1, 6), (IN1, 20), (12, IN1)] {
        g.blk(
            lx,
            ANTE_CEIL - 1,
            lz,
            "minecraft:lantern",
            Some(vec![("hanging", "true")]),
        );
    }
    // coiled bell-ropes hanging into the room (dressing, never a nav promise)
    for (rx, rz) in [(PIT0 - 2, PIT0 - 2), (PIT1 + 2, PIT1 + 2)] {
        for y in (KEEP_WALK + 2)..=(ANTE_CEIL - 1) {
            g.blk(
                rx,
                y,
                rz,
                "minecraft:iron_chain",
                Some(vec![("axis", "y"), ("waterlogged", "false")]),
            );
        }
    }

    // ---- 5. Stair 1: rope room -> loft --------------------------------------
    for z in 3..=15 {
        let w = stair1_walk(z);
        for x in S1_X0..=S1_X1 {
            g.carve(bx(x, x, w, w + 3, z, z));
            if z > 3 && stair1_walk(z - 1) > w {
                stairs(g, x, w - 1, z, "minecraft:stone_brick_stairs", "north");
            } else {
                g.blk(
                    x,
                    w - 1,
                    z,
                    pick(&keep_floor(), value_noise(seed, x, w, z, 0.3, 23)),
                    None,
                );
            }
        }
        if z % 4 == 0 {
            g.blk(
                S1_X0 - 1,
                stair1_walk(z) + 2,
                z,
                "minecraft:lantern",
                Some(vec![("hanging", "false")]),
            );
        }
    }

    // ---- 6. Bell loft: rafters + the four perches --------------------------
    // Two longitudinal purlins carry the perches; the transverse ties are kept
    // SHORT (and the mid-span tie is reduced to two wall braces) so no rafter ever
    // screens a perch from the loft doorway. The sightline proof below is what
    // pins this: change the rafter grid and it fails, loudly, at generation.
    for x in [6, 19] {
        g.fill(
            bx(x, x, RAFTER, RAFTER, IN0, IN1),
            "minecraft:dark_oak_log",
            Some(vec![("axis", "z")]),
        );
    }
    for z in [6, 18] {
        g.fill(
            bx(8, 17, RAFTER, RAFTER, z, z),
            "minecraft:dark_oak_log",
            Some(vec![("axis", "x")]),
        );
    }
    g.fill(
        bx(IN0, 4, RAFTER, RAFTER, 12, 12),
        "minecraft:dark_oak_log",
        Some(vec![("axis", "x")]),
    );
    g.fill(
        bx(21, IN1, RAFTER, RAFTER, 12, 12),
        "minecraft:dark_oak_log",
        Some(vec![("axis", "x")]),
    );
    // re-open the pit through the rafter grid so the shaft stays sheer
    g.carve(bx(PIT0, PIT1, RAFTER, RAFTER, PIT0, PIT1));
    // king posts, kept off every perch sightline (proved below)
    for (px, pz) in [(9, 9), (16, 9), (9, 16), (16, 16)] {
        g.fill(
            bx(px, px, LOFT_WALK, RAFTER - 1, pz, pz),
            "minecraft:dark_oak_log",
            Some(vec![("axis", "y")]),
        );
    }
    for (lx, lz) in [(IN0, 9), (IN1, 9), (IN0, 16), (IN1, 16)] {
        g.blk(
            lx,
            LOFT_WALK + 3,
            lz,
            "minecraft:lantern",
            Some(vec![("hanging", "false")]),
        );
    }
    // louvred openings: the loft is a belfry, so the sky is visible from it
    for z in (5..=20).step_by(3) {
        g.carve(bx(IN0 - 2, IN0 - 1, LOFT_WALK + 2, LOFT_WALK + 4, z, z));
        g.carve(bx(IN1 + 1, IN1 + 2, LOFT_WALK + 2, LOFT_WALK + 4, z, z));
    }

    // ---- 7. Stair 2: loft -> boss ring --------------------------------------
    for z in 3..=13 {
        let w = stair2_walk(z);
        for x in S2_X0..=S2_X1 {
            g.carve(bx(x, x, w, w + 3, z, z));
            if z > 3 && stair2_walk(z - 1) > w {
                stairs(g, x, w - 1, z, "minecraft:stone_brick_stairs", "north");
            } else {
                g.blk(
                    x,
                    w - 1,
                    z,
                    pick(&keep_floor(), value_noise(seed, x, w, z, 0.3, 25)),
                    None,
                );
            }
        }
        if z % 4 == 0 {
            g.blk(
                S2_X1 + 1,
                stair2_walk(z) + 2,
                z,
                "minecraft:lantern",
                Some(vec![("hanging", "false")]),
            );
        }
    }

    // ---- 8. Boss ring: pillar cover + the raised outer walk ----------------
    for (px, pz) in [(7, 7), (18, 7), (7, 18), (18, 18), (12, 5), (16, 12)] {
        g.fill(
            bx(px, px, RING_WALK, RING_CEIL - 1, pz, pz),
            "minecraft:chiseled_stone_bricks",
            None,
        );
    }
    // The raised outer walk is the SOUTH band, deliberately opposite the ring
    // doorway: the keeper is read from the doorway across open floor first, and
    // the high ground has to be crossed for — never handed out on entry.
    g.fill_pal(
        bx(IN0, IN1, RING_FLOOR + 2, RING_FLOOR + 2, 21, IN1),
        &keep_floor(),
        seed,
        0.3,
        27,
    );
    g.carve(bx(IN0, IN1, RING_FLOOR + 3, RING_CEIL, 21, IN1));
    g.blk(12, RING_FLOOR + 1, 20, "minecraft:stone_bricks", None);
    g.blk(13, RING_FLOOR + 1, 20, "minecraft:stone_bricks", None);
    g.carve(bx(12, 13, RING_FLOOR + 2, RING_CEIL, 20, 20));
    for (lx, lz) in [(IN0, 8), (IN1, 8), (IN0, 20), (IN1, 20), (12, IN1)] {
        g.blk(
            lx,
            RING_CEIL - 1,
            lz,
            "minecraft:lantern",
            Some(vec![("hanging", "true")]),
        );
    }
    // belfry openings: the ring is roofed but daylit through tall arches
    for z in [8, 12, 17, 21] {
        g.carve(bx(0, IN0 - 1, RING_WALK, RING_WALK + 4, z, z + 1));
        g.carve(bx(IN1 + 1, SX - 1, RING_WALK, RING_WALK + 4, z, z + 1));
        for y in RING_WALK..=(RING_WALK + 4) {
            g.blk(0, y, z, "minecraft:iron_bars", None);
            g.blk(0, y, z + 1, "minecraft:iron_bars", None);
            g.blk(SX - 1, y, z, "minecraft:iron_bars", None);
            g.blk(SX - 1, y, z + 1, "minecraft:iron_bars", None);
        }
    }

    // ---- 8b. Lighting ------------------------------------------------------
    // the two stairwells and the open pit are excluded: a lamp hung into a climb
    // walls the route off at head height under the nav model.
    let clear = |x: i32, z: i32| {
        ((S1_X0 - 1)..=(S1_X1 + 1)).contains(&x) && (2..=16).contains(&z)
            || ((S2_X0 - 1)..=(S2_X1 + 1)).contains(&x) && (2..=14).contains(&z)
            || ((PIT0 - 1)..=(PIT1 + 1)).contains(&x) && ((PIT0 - 1)..=(PIT1 + 1)).contains(&z)
            // the outer-walk ramp and the raised band it climbs to
            || (11..=14).contains(&x) && (19..=IN1).contains(&z)
    };
    light_room_ex(
        g,
        IN0,
        IN1,
        IN0,
        IN1,
        KEEP_WALK,
        ANTE_CEIL + 1,
        4,
        "minecraft:lantern",
        &clear,
    );
    light_room_ex(
        g,
        IN0,
        IN1,
        IN0,
        IN1,
        LOFT_WALK,
        LOFT_CEIL + 1,
        4,
        "minecraft:lantern",
        &clear,
    );
    light_room_ex(
        g,
        IN0,
        IN1,
        IN0,
        IN1,
        RING_WALK,
        RING_CEIL + 1,
        4,
        "minecraft:lantern",
        &clear,
    );
    // sconces use a NARROW exclusion (the stair treads and the void only) — the
    // wide chandelier exclusion would blank the room corners, which is exactly
    // where the measured minimum lands.
    let narrow = |x: i32, z: i32| {
        (S1_X0..=S1_X1).contains(&x) && (2..=16).contains(&z)
            || (S2_X0..=S2_X1).contains(&x) && (2..=14).contains(&z)
            || (PIT0..=PIT1).contains(&x) && (PIT0..=PIT1).contains(&z)
    };
    for w in [KEEP_WALK, LOFT_WALK, RING_WALK] {
        sconces(g, IN0, IN1, IN0, IN1, w + 3, 3, &narrow);
        sconces(g, IN0 + 1, IN1 - 1, IN0 + 1, IN1 - 1, w + 4, 4, &narrow);
    }
    // lamps slung under the two purlins — the loft's own light, and the thing
    // that throws the perched wardens into silhouette from the doorway
    for z in (4..=20).step_by(4) {
        for x in [6, 19] {
            if g.is_air(x, RAFTER - 1, z) && g.is_solid(x, RAFTER, z) {
                g.blk(
                    x,
                    RAFTER - 1,
                    z,
                    "minecraft:lantern",
                    Some(vec![("hanging", "true")]),
                );
            }
        }
    }
    // the pit rim itself: cressets on the kerb corners, so the arena floor around
    // the void is never the darkest thing in the room
    for (cx, cz) in [
        (PIT0 - 2, PIT0 - 2),
        (PIT1 + 2, PIT0 - 2),
        (PIT0 - 2, PIT1 + 2),
        (PIT1 + 2, PIT1 + 2),
    ] {
        for w in [KEEP_WALK, LOFT_WALK, RING_WALK] {
            if g.is_air(cx, w, cz) && g.is_solid(cx, w - 1, cz) {
                g.blk(cx, w, cz, "minecraft:sea_lantern", None);
            }
        }
    }
    // the raised outer walk sits above the sconce plane; light it from its own floor
    for x in (IN0 + 2..=IN1).step_by(5) {
        if g.is_air(x, RING_FLOOR + 3, IN1) && g.is_solid(x, RING_FLOOR + 2, IN1) {
            g.blk(
                x,
                RING_FLOOR + 3,
                IN1,
                "minecraft:lantern",
                Some(vec![("hanging", "false")]),
            );
        }
    }
    for z in (3..=15).step_by(4) {
        let w = stair1_walk(z);
        if g.is_air(S1_X0, w + 3, z) && g.is_solid(S1_X0 - 1, w + 3, z) {
            g.blk(
                S1_X0,
                w + 3,
                z,
                "minecraft:wall_torch",
                Some(vec![("facing", "east")]),
            );
        }
    }
    for z in (3..=13).step_by(4) {
        let w = stair2_walk(z);
        if g.is_air(S2_X1, w + 3, z) && g.is_solid(S2_X1 + 1, w + 3, z) {
            g.blk(
                S2_X1,
                w + 3,
                z,
                "minecraft:wall_torch",
                Some(vec![("facing", "west")]),
            );
        }
    }

    // ---- 8c. The crown -----------------------------------------------------
    // The belfry is OPEN to the sky: a bell tower that reads as a sealed brick
    // box from the courtyard is not a bell tower. Opening it also puts the boss
    // ring under daylight, which is the information-before-commitment the arena
    // wants (the keeper is seen from the doorway, not discovered in the dark).
    g.carve(bx(IN0, IN1, RING_CEIL, SY - 1, IN0, IN1));
    for x in 0..SX {
        for z in 0..TZ {
            let rim = !(IN0..=IN1).contains(&x) || !(IN0..=IN1).contains(&z);
            if !rim {
                continue;
            }
            let corner = !(IN0 + 1..=IN1 - 1).contains(&x) && !(IN0 + 1..=IN1 - 1).contains(&z);
            for y in RING_CEIL..=(SY - 4) {
                g.blk(
                    x,
                    y,
                    z,
                    pick(&keep_wall(), value_noise(seed, x, y, z, 0.2, 31)),
                    None,
                );
            }
            // corner turrets rise two courses over a crenellated curtain
            if corner {
                for y in (SY - 3)..SY {
                    g.blk(
                        x,
                        y,
                        z,
                        pick(&keep_wall(), value_noise(seed, x, y, z, 0.2, 33)),
                        None,
                    );
                }
            } else if (x + z) % 2 == 0 {
                g.blk(x, SY - 3, z, "minecraft:stone_bricks", None);
                g.blk(x, SY - 2, z, "minecraft:stone_brick_wall", None);
            } else {
                g.blk(x, SY - 3, z, "minecraft:stone_brick_wall", None);
            }
        }
    }
    // belfry arches on all four faces, not two — the louvres are the tower's
    // silhouette from every approach
    for k in [8, 12, 17, 21] {
        g.carve(bx(0, IN0 - 1, RING_WALK, RING_WALK + 4, k, k + 1));
        g.carve(bx(IN1 + 1, SX - 1, RING_WALK, RING_WALK + 4, k, k + 1));
        g.carve(bx(k, k + 1, RING_WALK, RING_WALK + 4, 0, IN0 - 1));
        g.carve(bx(k, k + 1, RING_WALK, RING_WALK + 4, IN1 + 1, TZ - 1));
        for y in RING_WALK..=(RING_WALK + 4) {
            for d in 0..2 {
                g.blk(0, y, k + d, "minecraft:iron_bars", None);
                g.blk(SX - 1, y, k + d, "minecraft:iron_bars", None);
                g.blk(k + d, y, 0, "minecraft:iron_bars", None);
                g.blk(k + d, y, TZ - 1, "minecraft:iron_bars", None);
            }
        }
    }
    // a string course marks the belfry line and breaks the flat brick face
    for x in 0..SX {
        for z in 0..TZ {
            if !(IN0..=IN1).contains(&x) || !(IN0..=IN1).contains(&z) {
                g.blk(x, LOFT_CEIL + 1, z, "minecraft:chiseled_stone_bricks", None);
                g.blk(x, ANTE_CEIL + 1, z, "minecraft:chiseled_stone_bricks", None);
            }
        }
    }

    // ---- 8d. Tide gate, sea-stair, ferry pier (r5) --------------------------
    // The road home. After the finale the party leaves through the rope room's
    // south wall and down the tower's own face — never back across the
    // courtyard the rest re-armed. Three parts, all inside this piece:
    //
    // * the **basin lip gap**: three kerb cells open toward the rope foot, so a
    //   player who takes the rope drop steps out of the water instead of
    //   treading it in a walled font. The nav model never routes through the
    //   basin (water is never a floor), so this changes nothing the compiler
    //   proves — it is the human exit the drop always needed;
    // * the **tide gate**: a 3×3 doorway through the south wall, shipped BARRED
    //   (iron bars — the sea reads through it from BF3, and no lever anywhere
    //   opens it; only the finale's `open-gate` clears the region);
    // * the **sea-stair and ferry pier**: a parapet-walled flight down the
    //   south face to a stone pier at the shore datum, where the ferrywoman
    //   moors for the ending.
    for z in 12..=14 {
        g.air(PIT0 - 1, KEEP_WALK, z);
    }
    for x in TG_X0..=TG_X1 {
        for y in KEEP_WALK..=(KEEP_WALK + 2) {
            for z in (TZ - 2)..TZ {
                g.blk(x, y, z, "minecraft:iron_bars", None);
            }
        }
    }
    // the sea band: seabed, two courses of water to the shore datum (waterline
    // local y=2 — the same ocean datum the barrow shore declares, `DW0344`),
    // seagrass in the shallows
    for x in 0..SX {
        for z in SEA_Z0..SZ {
            g.blk(
                x,
                0,
                z,
                pick(&plinth(), value_noise(seed, x, 0, z, 0.2, 41)),
                None,
            );
            g.blk(x, 1, z, "minecraft:water", None);
            g.blk(x, 2, z, "minecraft:water", None);
            if value_noise(seed, x, 1, z, 0.4, 43) > 0.82 {
                g.blk(x, 1, z, "minecraft:seagrass", None);
            }
        }
    }
    // the stair: one tread per z from the keep walk down to the shore walk,
    // solid to the seabed, parapet-walled on both flanks (the parapet is also
    // what `seal_stair_flanks` would otherwise have to invent)
    for z in SEA_Z0..=SEA_Z1 {
        let w = sea_stair_walk(z);
        for x in TG_X0..=TG_X1 {
            for y in 0..=(w - 2) {
                g.blk(
                    x,
                    y,
                    z,
                    pick(&keep_wall(), value_noise(seed, x, y, z, 0.11, 45)),
                    None,
                );
            }
            stairs(g, x, w - 1, z, "minecraft:stone_brick_stairs", "north");
        }
        for x in [TG_X0 - 1, TG_X1 + 1] {
            for y in 0..=(w - 1) {
                g.blk(
                    x,
                    y,
                    z,
                    pick(&keep_wall(), value_noise(seed, x, y, z, 0.11, 47)),
                    None,
                );
            }
            g.blk(x, w, z, "minecraft:stone_brick_wall", None);
        }
    }
    // the pier deck, at the barrow-shore datum (floor top = waterline)
    for x in PIER_X0..=PIER_X1 {
        for z in PIER_Z0..SZ {
            for y in 0..=1 {
                g.blk(
                    x,
                    y,
                    z,
                    pick(&plinth(), value_noise(seed, x, y, z, 0.16, 49)),
                    None,
                );
            }
            g.blk(
                x,
                2,
                z,
                pick(&keep_floor(), value_noise(seed, x, 2, z, 0.2, 51)),
                None,
            );
        }
    }
    // parapet lanterns on the flight, mooring posts + lanterns on the deck —
    // the walk's own light (the compiler re-measures and relights assembled,
    // spec-0010; this keeps the authored minimum honest)
    for z in [SEA_Z0 + 1, SEA_Z0 + 4] {
        let w = sea_stair_walk(z);
        for x in [TG_X0 - 1, TG_X1 + 1] {
            g.blk(
                x,
                w + 1,
                z,
                "minecraft:lantern",
                Some(vec![("hanging", "false")]),
            );
        }
    }
    for (px, pz) in [
        (PIER_X0, PIER_Z0),
        (PIER_X1, PIER_Z0),
        (PIER_X0, SZ - 1),
        (PIER_X1, SZ - 1),
    ] {
        g.blk(px, SHORE_WALK, pz, "minecraft:oak_fence", None);
        g.blk(
            px,
            SHORE_WALK + 1,
            pz,
            "minecraft:lantern",
            Some(vec![("hanging", "false")]),
        );
    }
    // a driftwood spar beached on the deck's west end — the boat's oar-side
    for k in 0..3 {
        g.blk(
            PIER_X0 + 1 + k,
            SHORE_WALK,
            SZ - 2,
            "minecraft:oak_log",
            Some(vec![("axis", "x")]),
        );
    }

    // ---- 9. Socket ----------------------------------------------------------
    cut_socket(g, Side::West, KEEP_FLOOR_Y, 13);
    g.carve(bx(0, IN0 - 1, KEEP_WALK, KEEP_WALK + 2, 12, 14));

    // ---- 10. Invariants -----------------------------------------------------
    // Both flights rise through OPEN room air, so their side rails are whatever
    // happens to sit one block under a tread — the rope-room floor, the loft
    // floor, and (at z=7) the x=19 purlin itself. Five newels; without them the
    // route side-steps into mid-flight and `DW0430` reports a tread that is
    // asked to carry two climbs at once.
    seal_stair_flanks(g, "minecraft:stone_brick_wall");

    let mut climb: Vec<[i32; 3]> = Vec::new();
    for z in (16..=20).rev() {
        climb.push([4, KEEP_WALK, z]);
    }
    for z in (3..=15).rev() {
        climb.push([4, stair1_walk(z), z]);
    }
    for x in 5..=17 {
        climb.push([x, LOFT_WALK, 3]);
    }
    for z in 4..=13 {
        climb.push([17, LOFT_WALK, z]);
    }
    for x in 18..=21 {
        climb.push([x, LOFT_WALK, 13]);
    }
    for z in (3..=12).rev() {
        climb.push([21, stair2_walk(z), z]);
    }
    assert_route_walkable("tk-bell-tower", "rope room -> loft -> boss ring", g, &climb);

    let outer = vec![
        [12, RING_WALK, 19],
        [12, RING_WALK + 1, 20],
        [12, RING_WALK + 2, 21],
        [12, RING_WALK + 2, 22],
    ];
    assert_route_walkable("tk-bell-tower", "raised outer walk ramp", g, &outer);

    // r5: the road home. The gate cells themselves ship barred, so the proof is
    // in two halves on either side of the sealed region — the compiler's own
    // DAG-causal nav proves the joined route once the finale opens the gate.
    assert_route_walkable(
        "tk-bell-tower",
        "rope room -> tide gate (inner approach)",
        g,
        &[
            [13, KEEP_WALK, 20],
            [13, KEEP_WALK, 21],
            [13, KEEP_WALK, 22],
            [13, KEEP_WALK, 23],
        ],
    );
    let mut sea: Vec<[i32; 3]> = Vec::new();
    for z in SEA_Z0..=SEA_Z1 {
        sea.push([13, sea_stair_walk(z), z]);
    }
    for z in PIER_Z0..SZ {
        sea.push([13, SHORE_WALK, z]);
    }
    assert_route_walkable(
        "tk-bell-tower",
        "tide gate -> sea-stair -> ferry pier",
        g,
        &sea,
    );
    assert_route_walkable(
        "tk-bell-tower",
        "basin lip gap -> rope foot",
        g,
        &[[10, KEEP_WALK, 12], [10, KEEP_WALK, 13], [9, KEEP_WALK, 13]],
    );

    for (i, p) in PERCHES.iter().enumerate() {
        assert!(
            standable(g, *p),
            "tk-bell-tower: rafter perch {} at {p:?} is not standable",
            i + 1
        );
        assert!(
            // eye-to-silhouette: what must be visible is the standing warden's
            // body, not its boots — a perch cell can legitimately be screened by
            // the beam it rests on.
            sightline_clear(
                g,
                [LOFT_DOOR[0], LOFT_DOOR[1] + 1, LOFT_DOOR[2]],
                [p[0], p[1] + 1, p[2]]
            ),
            "tk-bell-tower: rafter perch {} at {p:?} is NOT visible from the loft doorway — the \
             TWIST ambush is fair by sightline alone; an unseen perch makes it a cheat",
            i + 1
        );
    }
}

/// A conservative voxel sightline test (DDA-free supersampling along the segment):
/// every sampled cell strictly between the two endpoints must be non-opaque.
fn sightline_clear(g: &Grid, from: [i32; 3], to: [i32; 3]) -> bool {
    let d = [
        (to[0] - from[0]) as f64,
        (to[1] - from[1]) as f64,
        (to[2] - from[2]) as f64,
    ];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    let steps = (len * 4.0).ceil() as i32;
    for s in 1..steps {
        let t = s as f64 / steps as f64;
        let c = [
            (from[0] as f64 + d[0] * t + 0.5).floor() as i32,
            (from[1] as f64 + d[1] * t + 0.5).floor() as i32,
            (from[2] as f64 + d[2] * t + 0.5).floor() as i32,
        ];
        if c == from || c == to {
            continue;
        }
        if !g.inb(c[0], c[1], c[2]) {
            return false;
        }
        if !transparent(g.get(c[0], c[1], c[2])) {
            return false;
        }
    }
    true
}

pub fn anchors() -> Vec<(&'static str, AnchorJson)> {
    let mut v = vec![
        ("anchor/l4-rope-room", a_pos([5, KEEP_WALK, 18], "east")),
        ("anchor/l4-bonfire", a_pos([8, KEEP_WALK, 20], "north")),
        ("anchor/l4-rope-foot", a_pos([9, KEEP_WALK, 13], "east")),
        ("anchor/l4-loft-door", a_pos(LOFT_DOOR, "south")),
        ("anchor/l4-loft", a_pos([12, LOFT_WALK, 20], "north")),
        ("anchor/l4-ring-door", a_pos([21, RING_WALK, 3], "south")),
        ("anchor/l4-boss", a_pos([12, RING_WALK, 17], "north")),
        ("anchor/l4-ring-west", a_pos([4, RING_WALK, 12], "east")),
        ("anchor/l4-ring-east", a_pos([21, RING_WALK, 12], "west")),
        (
            "anchor/l4-outer-walk",
            a_pos([12, RING_WALK + 2, 22], "north"),
        ),
        ("anchor/l4-vantage", a_pos([5, RING_WALK + 2, 22], "north")),
        (
            "anchor/l4-rope-drop",
            a_region(
                [PIT0, RING_WALK, PIT0],
                [PIT1, RING_WALK, PIT1],
                "minecraft:iron_bars",
            ),
        ),
        ("anchor/l4-bell-hang", a_pos([12, RING_WALK, 9], "north")),
        (
            "anchor/l4-tide-gate",
            a_region(
                [TG_X0, KEEP_WALK, TZ - 2],
                [TG_X1, KEEP_WALK + 2, TZ - 1],
                "minecraft:iron_bars",
            ),
        ),
        ("anchor/l4-pier", a_pos([13, SHORE_WALK, SZ - 2], "north")),
    ];
    for (i, p) in PERCHES.iter().enumerate() {
        let name: &'static str = match i {
            0 => "anchor/l4-perch-1",
            1 => "anchor/l4-perch-2",
            2 => "anchor/l4-perch-3",
            _ => "anchor/l4-perch-4",
        };
        v.push((name, a_pos(*p, "south")));
    }
    v
}

pub fn light_regions() -> Vec<[i32; 6]> {
    vec![
        bx(IN0, IN1, KEEP_WALK, KEEP_WALK + 1, IN0, IN1),
        bx(IN0, IN1, LOFT_WALK, LOFT_WALK + 1, IN0, IN1),
        bx(IN0, IN1, RING_WALK, RING_WALK + 1, IN0, IN1),
        // r5: the sea-stair treads and the ferry pier deck
        bx(TG_X0, TG_X1, 4, KEEP_WALK - 1, SEA_Z0, SEA_Z1),
        bx(
            PIER_X0,
            PIER_X1,
            SHORE_WALK,
            SHORE_WALK + 1,
            PIER_Z0,
            SZ - 1,
        ),
    ]
}
