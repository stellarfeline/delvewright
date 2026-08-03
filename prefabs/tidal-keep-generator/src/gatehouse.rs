//! `tk-gatehouse` (L1) — the timed portcullis and the boulder stair.
//!
//! Three beats in one piece, because the solver assembles a TREE (every unmated
//! socket is walled up), so a fork that rejoins can only be authored *inside* a
//! prefab:
//!
//! 1. **The timed gate** — a portcullis region on the approach, with a roofed
//!    watch bay set into the east wall of the court six blocks out, holding clean
//!    line of sight up the passage so the full cycle is readable BEFORE anyone
//!    commits (spec-0016 §4 wants a timing read, not a coin flip).
//! 2. **The boulder stair — 初见杀 #1** — a long straight run whose centre lane
//!    has been polished smooth by a century of stone (the palette IS the tell:
//!    worn `smooth_stone`/`polished_andesite` down the middle, untrodden mossy
//!    and cracked brick on the flanks). A plate row mid-run fires a dispenser set
//!    into the arch rib above and ahead. The runout alcove at the head of the
//!    stair holds the **spill shaft** — a one-way drop into the gatehouse
//!    undercroft's water trough that returns the player to the ward and BF1. The
//!    thing that kills you IS the shortcut you learn.
//! 3. **The mural stair** — a narrow flank climb up the east wall, the
//!    boulder-free counterplay, paid for with an ambush doorway at mid-height.
//!
//! Vanilla ladders are deliberately NOT used for the descent: the compiler's nav
//! model does not model climbables at all (a ladder is just a solid obstacle), so
//! a ladder route would be an unprovable promise. A drop into water is one-way by
//! geometry, damage-free, and honest about what the engine can see.

use crate::common::*;

pub const SX: i32 = 28;
pub const SY: i32 = 24;
pub const SZ: i32 = 46;

const SHELL_TOP: i32 = 18;
const TOWER_TOP: i32 = 22;

/// Portcullis plane.
const GATE_Z: i32 = 36;
/// Boulder-stair corridor (x range) and its z run.
const ST_X0: i32 = 11;
const ST_X1: i32 = 16;
const ST_Z0: i32 = 10;
const ST_Z1: i32 = 29;
/// Mural (flank) stair.
const MU_X0: i32 = 21;
const MU_X1: i32 = 23;
const MU_Z0: i32 = 9;
const MU_Z1: i32 = 28;
/// Undercroft gallery under the stair head (the spill shaft's landing).
const GA_X0: i32 = 4;
const GA_X1: i32 = 8;
const GA_Z0: i32 = 3;
const GA_Z1: i32 = 28;
/// Trap geometry.
const PLATE_Z: i32 = 18;
const RIB_Z: i32 = 15;
const DISP: [i32; 3] = [14, 12, RIB_Z];

/// Walk height of the boulder stair at depth `z` (monotone, one-block risers).
pub fn stair_walk(z: i32) -> i32 {
    SHORE_WALK + ((30 - z) * 8 + 19) / 20
}
/// Walk height of the mural flank stair at depth `z`.
fn mural_walk(z: i32) -> i32 {
    SHORE_WALK + ((28 - z) * 8 + 17) / 18
}

pub fn build(g: &mut Grid, seed: u64) {
    // ---- 1. Solid massing (fill, then carve) --------------------------------
    for x in 0..SX {
        for z in 0..SZ {
            let tower = (2..=8).contains(&x) || (19..=25).contains(&x);
            let top = if tower && (32..=44).contains(&z) {
                TOWER_TOP
            } else {
                SHELL_TOP
            };
            for y in 0..=top {
                let name = if y < SHORE_FLOOR_Y {
                    pick(&plinth(), value_noise(seed, x, y, z, 0.15, 11))
                } else {
                    pick(&keep_wall(), value_noise(seed, x, y, z, 0.13, 13))
                };
                g.blk(x, y, z, name, None);
            }
        }
    }

    // ---- 2. The approach court (open to sky) + watch bay + portcullis -------
    g.carve(bx(8, 21, SHORE_WALK, SY - 1, GATE_Z + 2, SZ - 1));
    // watch bay: a roofed recess in the east wall, six blocks out from the gate,
    // sighting straight up the passage.
    g.carve(bx(19, 21, SHORE_WALK, SHORE_WALK + 2, 40, 43));
    g.fill(
        bx(19, 21, SHORE_WALK + 3, SHORE_WALK + 3, 40, 43),
        "minecraft:stone_bricks",
        None,
    );
    g.blk(
        21,
        SHORE_WALK + 2,
        41,
        "minecraft:lantern",
        Some(vec![("hanging", "false")]),
    );
    // the gate tunnel itself (5 wide, 3 tall) and the portcullis housing slot
    g.carve(bx(12, 16, SHORE_WALK, SHORE_WALK + 2, GATE_Z, GATE_Z + 1));
    g.carve(bx(12, 16, SHORE_WALK + 3, SHORE_WALK + 5, GATE_Z, GATE_Z));
    g.fill(
        bx(12, 16, SHORE_WALK + 3, SHORE_WALK + 5, GATE_Z, GATE_Z),
        "minecraft:air",
        None,
    );
    // the bars themselves are placed by the campaign's gate verbs; the prefab
    // ships the slot they retract into, so the cycle reads as a real portcullis.

    // ---- 3. The lower ward --------------------------------------------------
    g.carve(bx(4, 23, SHORE_WALK, SHORE_WALK + 6, 29, 35));
    for x in [6, 12, 18, 22] {
        g.blk(
            x,
            SHORE_WALK + 5,
            32,
            "minecraft:lantern",
            Some(vec![("hanging", "true")]),
        );
    }
    // ward dressing: a spoil heap and two broken pillars
    for (px, pz) in [(9, 31), (19, 34)] {
        for y in SHORE_WALK..=(SHORE_WALK + 2) {
            g.blk(px, y, pz, "minecraft:mossy_stone_bricks", None);
        }
    }

    // ---- 4. The boulder stair ----------------------------------------------
    for z in ST_Z0..=ST_Z1 {
        let w = stair_walk(z);
        let top = w - 1;
        for x in ST_X0..=ST_X1 {
            // clear the shaft above the tread (3 air: feet, head, jump sweep)
            g.carve(bx(x, x, w, w + 4, z, z));
            // the wear gradient IS the tell: the centre lane the stone rolls down
            // is polished featureless; the flanks keep their mossy brick face.
            let worn = (x - (ST_X0 + ST_X1) / 2).abs() <= 1;
            let rise = z < ST_Z1 && stair_walk(z + 1) < w;
            if rise {
                stairs(
                    g,
                    x,
                    top,
                    z,
                    if worn {
                        "minecraft:stone_stairs"
                    } else {
                        "minecraft:mossy_stone_brick_stairs"
                    },
                    "north",
                );
            } else {
                let pal = if worn { tread_worn() } else { tread_unworn() };
                g.blk(
                    x,
                    top,
                    z,
                    pick(&pal, value_noise(seed, x, top, z, 0.25, 21)),
                    None,
                );
            }
        }
        if z % 5 == 0 {
            g.blk(
                ST_X0,
                stair_walk(z) + 3,
                z,
                "minecraft:lantern",
                Some(vec![("hanging", "true")]),
            );
        }
    }

    // ---- 5. The stair head landing + runout alcove + spill shaft ------------
    g.carve(bx(4, 23, KEEP_WALK, KEEP_WALK + 6, 2, 9));
    g.fill_pal(
        bx(4, 23, KEEP_FLOOR_Y, KEEP_FLOOR_Y, 2, 9),
        &keep_floor(),
        seed,
        0.2,
        23,
    );
    for x in [7, 13, 20] {
        g.blk(
            x,
            KEEP_WALK + 5,
            5,
            "minecraft:lantern",
            Some(vec![("hanging", "true")]),
        );
    }
    // the runout alcove: a deep west recess you dive into when the stone comes
    g.carve(bx(4, 8, KEEP_WALK, KEEP_WALK + 3, 3, 7));
    g.blk(
        4,
        KEEP_WALK + 2,
        5,
        "minecraft:lantern",
        Some(vec![("hanging", "false")]),
    );
    // the spill shaft: a 2x2 well from the alcove floor into the undercroft
    g.carve(bx(5, 6, 1, KEEP_FLOOR_Y, 4, 5));
    g.fill(bx(5, 6, 1, SHORE_FLOOR_Y, 4, 5), "minecraft:water", None);
    for x in 4..=7 {
        for z in 3..=6 {
            if !(5..=6).contains(&x) || !(4..=5).contains(&z) {
                g.blk(x, SHORE_FLOOR_Y, z, "minecraft:mossy_cobblestone", None);
            }
        }
    }
    // a stone kerb marks the well lip so the drop is SEEN before it is taken
    for (kx, kz) in [
        (4, 4),
        (4, 5),
        (7, 4),
        (7, 5),
        (5, 3),
        (6, 3),
        (5, 6),
        (6, 6),
    ] {
        g.blk(kx, KEEP_WALK, kz, "minecraft:stone_brick_wall", None);
    }

    // ---- 6. The undercroft gallery (spill landing -> ward) ------------------
    g.carve(bx(GA_X0, GA_X1, SHORE_WALK, SHORE_WALK + 5, GA_Z0, GA_Z1));
    g.fill_pal(
        bx(GA_X0, GA_X1, SHORE_FLOOR_Y, SHORE_FLOOR_Y, GA_Z0, GA_Z1),
        &tide_floor(),
        seed,
        0.2,
        25,
    );
    // re-open the well column through the freshly-laid gallery floor
    g.carve(bx(5, 6, 1, SHORE_WALK + 5, 4, 5));
    g.fill(bx(5, 6, 1, SHORE_FLOOR_Y, 4, 5), "minecraft:water", None);
    for z in (GA_Z0 + 3..=GA_Z1).step_by(7) {
        g.blk(
            GA_X0,
            SHORE_WALK + 4,
            z,
            "minecraft:soul_lantern",
            Some(vec![("hanging", "true")]),
        );
        g.blk(
            GA_X1,
            SHORE_WALK + 4,
            z,
            "minecraft:lantern",
            Some(vec![("hanging", "true")]),
        );
    }

    // ---- 7. The mural (flank) stair + its ambush doorway --------------------
    for z in MU_Z0..=MU_Z1 {
        let w = mural_walk(z);
        for x in MU_X0..=MU_X1 {
            g.carve(bx(x, x, w, w + 3, z, z));
            let rise = z < MU_Z1 && mural_walk(z + 1) < w;
            if rise {
                stairs(g, x, w - 1, z, "minecraft:stone_brick_stairs", "north");
            } else {
                g.blk(
                    x,
                    w - 1,
                    z,
                    pick(&keep_floor(), value_noise(seed, x, w - 1, z, 0.22, 27)),
                    None,
                );
            }
        }
        if z % 6 == 0 {
            g.blk(
                MU_X0,
                mural_walk(z) + 3,
                z,
                "minecraft:lantern",
                Some(vec![("hanging", "false")]),
            );
        }
    }
    // the ambush doorway: a dark side chamber opening onto the flank climb
    let amb_w = mural_walk(18);
    g.carve(bx(24, 26, amb_w, amb_w + 2, 17, 19));
    g.blk(
        26,
        amb_w + 2,
        18,
        "minecraft:soul_lantern",
        Some(vec![("hanging", "false")]),
    );

    // ---- 8. Trap hardware: the boulder run ---------------------------------
    let plate_w = stair_walk(PLATE_Z);
    for x in 12..=16 {
        plate(g, x, plate_w, PLATE_Z);
    }
    // the arch rib the charge is set into, with its two piers
    g.fill_pal(
        bx(ST_X0, 17, 12, 12, RIB_Z, RIB_Z),
        &keep_wall(),
        seed,
        0.3,
        29,
    );
    g.fill_pal(
        bx(16, 17, 11, 13, RIB_Z, RIB_Z),
        &keep_wall(),
        seed,
        0.3,
        31,
    );
    g.fill_pal(
        bx(ST_X0, 12, 11, 13, RIB_Z, RIB_Z),
        &keep_wall(),
        seed,
        0.3,
        33,
    );
    dispenser(g, DISP, "south");
    // wall duct for the wire (carved after the rib so the slot survives)
    for (cx, cy, cz) in [
        (17, 8, 18),
        (17, 9, 18),
        (17, 9, 17),
        (17, 10, 17),
        (17, 10, 16),
        (17, 11, 16),
        (17, 11, 15),
        (17, 12, 15),
        (16, 13, 15),
        (15, 14, 15),
        (14, 14, 15),
    ] {
        g.air(cx, cy, cz);
    }
    wire_dust(
        "tk-gatehouse",
        g,
        &[
            [17, 8, PLATE_Z],
            [17, 9, 17],
            [17, 10, 16],
            [17, 11, RIB_Z],
            [16, 12, RIB_Z],
            [15, 13, RIB_Z],
            [14, 13, RIB_Z],
        ],
    );

    // ---- 8b. Lighting ------------------------------------------------------
    let clear = |x: i32, z: i32| {
        ((ST_X0 - 1)..=(ST_X1 + 1)).contains(&x) && ((ST_Z0 - 1)..=(ST_Z1 + 1)).contains(&z)
            || ((MU_X0 - 1)..=(MU_X1 + 1)).contains(&x) && ((MU_Z0 - 1)..=(MU_Z1 + 1)).contains(&z)
            || (4..=8).contains(&x) && (3..=7).contains(&z)
    };
    light_room_ex(
        g,
        4,
        23,
        29,
        35,
        SHORE_WALK,
        10,
        4,
        "minecraft:lantern",
        &clear,
    );
    light_room_ex(
        g,
        4,
        23,
        2,
        9,
        KEEP_WALK,
        17,
        3,
        "minecraft:lantern",
        &clear,
    );
    sconces(g, 4, 23, 2, 9, KEEP_WALK + 3, 3, &clear);
    sconces(g, 4, 23, 29, 35, SHORE_WALK + 3, 3, &clear);
    light_room(
        g,
        GA_X0,
        GA_X1,
        GA_Z0,
        GA_Z1,
        SHORE_WALK,
        SHORE_WALK + 6,
        5,
        "minecraft:soul_lantern",
    );
    // Sconces on a climbing stair are set INTO the wall as embedded lamps rather
    // than hung on its face: the run's head-sweep cell moves with every riser, so
    // a face-mounted torch is one carve away from standing in the climb.
    let mut lit_treads = 0;
    for z in (ST_Z0..=ST_Z1).step_by(2) {
        let w = stair_walk(z);
        for wx in [ST_X0 - 1, ST_X1 + 1] {
            if g.is_solid(wx, w + 2, z) {
                g.blk(wx, w + 2, z, "minecraft:sea_lantern", None);
                lit_treads += 1;
            }
        }
    }
    assert!(
        lit_treads >= 16,
        "tk-gatehouse: only {lit_treads} stair lamps seated — the boulder run must be lit end to end"
    );
    for z in (MU_Z0..=MU_Z1).step_by(3) {
        let w = mural_walk(z);
        if g.is_air(MU_X0, w + 3, z) && g.is_solid(MU_X0 - 1, w + 3, z) {
            g.blk(
                MU_X0,
                w + 3,
                z,
                "minecraft:wall_torch",
                Some(vec![("facing", "east")]),
            );
        }
    }

    // ---- 9. Sockets ---------------------------------------------------------
    g.carve(bx(13, 15, KEEP_WALK, KEEP_WALK + 2, 0, 1));
    cut_socket(g, Side::South, SHORE_FLOOR_Y, 14);
    g.carve(bx(13, 15, SHORE_WALK, SHORE_WALK + 2, 44, 45));
    cut_socket(g, Side::North, KEEP_FLOOR_Y, 14);

    // ---- 10. Crenellations on the two gate towers --------------------------
    for x in 2..=25 {
        for z in [32, 44] {
            if !(9..=18).contains(&x) && (x + z) % 2 == 0 {
                g.blk(x, TOWER_TOP + 1, z, "minecraft:stone_brick_wall", None);
            }
        }
    }

    // ---- 11. Invariants -----------------------------------------------------
    // The boulder stair is carved column-by-column and needs nothing, but the
    // mural flank's TOP tread comes out flush beside the keep plinth, so the
    // plinth is a one-block side entry onto it. One newel closes it.
    seal_stair_flanks(g, "minecraft:mossy_stone_brick_wall");

    let mut stair_route: Vec<[i32; 3]> = Vec::new();
    for z in (ST_Z0..=35).rev() {
        let w = if z > ST_Z1 { SHORE_WALK } else { stair_walk(z) };
        stair_route.push([14, w, z]);
    }
    for z in (2..ST_Z0).rev() {
        stair_route.push([14, KEEP_WALK, z]);
    }
    stair_route.reverse();
    assert_route_walkable(
        "tk-gatehouse",
        "boulder stair (centre lane)",
        g,
        &stair_route,
    );

    let mut mural_route: Vec<[i32; 3]> = Vec::new();
    for z in (MU_Z0..=30).rev() {
        let w = if z > MU_Z1 { SHORE_WALK } else { mural_walk(z) };
        mural_route.push([22, w, z]);
    }
    mural_route.reverse();
    assert_route_walkable("tk-gatehouse", "mural flank stair", g, &mural_route);

    let gallery: Vec<[i32; 3]> = (GA_Z0 + 3..=GA_Z1).map(|z| [7, SHORE_WALK, z]).collect();
    assert_route_walkable("tk-gatehouse", "undercroft gallery", g, &gallery);
}

pub fn anchors() -> Vec<(&'static str, AnchorJson)> {
    let plate_w = stair_walk(PLATE_Z);
    vec![
        ("anchor/l1a-approach", a_pos([14, SHORE_WALK, 42], "north")),
        ("anchor/l1a-watch", a_pos([20, SHORE_WALK, 42], "north")),
        (
            "anchor/l1a-watch-corpse",
            a_pos([20, SHORE_WALK, 40], "west"),
        ),
        (
            "anchor/l1a-gate-timed",
            a_region(
                [12, SHORE_WALK, GATE_Z],
                [16, SHORE_WALK + 2, GATE_Z],
                "minecraft:iron_bars",
            ),
        ),
        ("anchor/l1a-ward", a_pos([14, SHORE_WALK, 32], "north")),
        (
            "anchor/l1a-stair-foot",
            a_pos([14, stair_walk(29), 29], "north"),
        ),
        (
            "anchor/l1a-trap-boulder",
            a_trap([14, plate_w, PLATE_Z], "north", DISP, PLATE_BLOCK),
        ),
        // spec-0022 traps v2: the consequence is a command payload, so the run
        // needs a slot to fire FROM and a box to blanket. The slot is the arch
        // rib's own opening one course under the dispenser — the murder-hole
        // that was always in the masonry; the dispenser stays as its visible
        // scenery. Deliberately NOT the dispenser cell itself, which is solid
        // (`DW0446`).
        (
            "anchor/l1a-volley-slot",
            a_slot([14, DISP[1] - 1, RIB_Z], "south"),
        ),
        (
            "anchor/l1a-stair-run",
            a_pos([14, stair_walk(19), 19], "north"),
        ),
        ("anchor/l1a-stair-head", a_pos([14, KEEP_WALK, 9], "north")),
        ("anchor/l1a-runout", a_pos([7, KEEP_WALK, 6], "west")),
        ("anchor/l1a-spill-shaft", a_pos([8, KEEP_WALK, 5], "west")),
        ("anchor/l1a-undercroft", a_pos([7, SHORE_WALK, 8], "south")),
        (
            "anchor/l1a-mural-foot",
            a_pos([22, SHORE_WALK, 29], "north"),
        ),
        (
            "anchor/l1a-mural-door",
            a_pos([25, mural_walk(18), 18], "west"),
        ),
        ("anchor/l1a-roof-door", a_pos([14, KEEP_WALK, 3], "north")),
    ]
}

/// Interior volumes the derived light estimate is measured over.
pub fn light_regions() -> Vec<[i32; 6]> {
    vec![
        bx(4, 23, SHORE_WALK, SHORE_WALK + 1, 29, 35),
        bx(ST_X0, ST_X1, SHORE_WALK, KEEP_WALK, ST_Z0, ST_Z1),
        bx(4, 23, KEEP_WALK, KEEP_WALK + 1, 2, 9),
        bx(GA_X0, GA_X1, SHORE_WALK, SHORE_WALK + 1, GA_Z0, GA_Z1),
        bx(MU_X0, MU_X1, SHORE_WALK, KEEP_WALK, MU_Z0, MU_Z1),
    ]
}
