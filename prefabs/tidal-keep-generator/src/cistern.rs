//! `tk-cistern` (L3) — the undercroft, and the local loop that opens shortcut A.
//!
//! The souls loop is authored ENTIRELY inside this piece, because the solver
//! assembles a tree: a sealed door on one prefab can never reopen onto another.
//! So the vestibule (one step off the courtyard) forks:
//!
//! * **long route** — the commit gate, the descent stair, the flooded undercroft
//!   (pillar ambush, item alcove, the drowned secret), the east-wall ledge line,
//!   the dart gallery over the exit stair, and finally the far-side landing where
//!   the unlock sits;
//! * **short route** — shortcut door A, sealed from world-load, opening onto a
//!   straight upper gallery from the vestibule to that same landing.
//!
//! Water is authored as **sunken bays whose surface sits level with the dry
//!  floor** — it reads as a flooded undercroft, and every proven route stays dry.
//! That is not a stylistic choice: the compiler's nav model treats water as
//! impassable and never a floor (`standable` requires a SOLID block below), so a
//! wading route could not be proven at all. The bays are the hazard/attrition
//! zone; the causeways and the ledge are the walk.

use crate::common::*;

pub const SX: i32 = 42;
pub const SY: i32 = 22;
pub const SZ: i32 = 40;

/// Undercroft datum.
const U_FLOOR: i32 = 3;
const U_WALK: i32 = 4;
const U_CEIL: i32 = 9;
const U_Z0: i32 = 26;
const U_Z1: i32 = 38;
/// The flooded bay (sunken floor; the water surface is level with `U_FLOOR`).
const BAY: [i32; 6] = [8, 33, 1, U_FLOOR, 30, 35];
/// Upper deck rooms.
const VES: [i32; 4] = [1, 9, 14, 24]; // x0,x1,z0,z1
const GAL: [i32; 4] = [10, 30, 18, 20];
const LND: [i32; 4] = [31, 40, 14, 24];
const UP_CEIL: i32 = 15;
/// Stairwells. Each is built as a SOLID envelope first and then carved, because
/// the undercroft hall spans the full width: carving the hall before the wells
/// would leave their side walls (and the wire duct behind them) as open air.
const DS_X0: i32 = 2;
const DS_X1: i32 = 4;
const DS_Z0: i32 = 25;
const DS_Z1: i32 = 32;
const EX_X0: i32 = 37;
const EX_X1: i32 = 39;
const EX_Z0: i32 = 24;
const EX_Z1: i32 = 33;
/// East-wall ledge line (raised over the bay).
const LEDGE_X: i32 = 34;
/// Dart trap: the plate row, and the dispenser two blocks over the tread.
const DART_Z: i32 = 30;
const DART_DISP: [i32; 3] = [EX_X1, 9, 28];
/// The duct column behind the stair's east wall that carries the wire.
const DUCT_X: i32 = EX_X1 + 1;

fn descent_walk(z: i32) -> i32 {
    KEEP_WALK - (z - DS_Z0)
}
/// The exit climb, with a deliberate two-tread LANDING at z=28..29. The flat
/// section is what lets the wire duct out-climb the stair and put the dart
/// gallery genuinely overhead instead of at ankle height — geometry serving the
/// hardware, rather than the hardware being fudged.
fn exit_walk(z: i32) -> i32 {
    match z {
        33 => 4,
        32 => 5,
        31 => 6,
        28..=30 => 7,
        27 => 8,
        26 => 9,
        25 => 10,
        _ => KEEP_WALK,
    }
}
/// The ledge undulates by one block — free, but never flat (`|dy| <= 1`, and a
/// full-block rise gets its head-sweep cell proved clear).
fn ledge_floor(z: i32) -> i32 {
    if z == 30 || z == 33 {
        U_FLOOR + 2
    } else {
        U_FLOOR + 1
    }
}

pub fn build(g: &mut Grid, seed: u64) {
    // ---- 1. Solid rock ------------------------------------------------------
    for x in 0..SX {
        for z in 0..SZ {
            for y in 0..SY {
                let name = if y <= U_FLOOR {
                    pick(&plinth(), value_noise(seed, x, y, z, 0.14, 11))
                } else {
                    pick(&tide_wall(), value_noise(seed, x, y, z, 0.12, 13))
                };
                g.blk(x, y, z, name, None);
            }
        }
    }

    // ---- 2. The undercroft hall + its flooded bay --------------------------
    g.carve(bx(1, SX - 2, U_WALK, U_CEIL, U_Z0, U_Z1));
    g.fill_pal(
        bx(1, SX - 2, U_FLOOR, U_FLOOR, U_Z0, U_Z1),
        &tide_floor(),
        seed,
        0.2,
        15,
    );
    // sink the bay and fill it so its surface is level with the dry floor top
    g.carve(bx(BAY[0], BAY[1], BAY[2], BAY[3], BAY[4], BAY[5]));
    // the bay bed is the piece's BOTTOM layer, so it carries no gravity block
    g.fill_pal(
        bx(BAY[0], BAY[1], 0, 0, BAY[4], BAY[5]),
        &plinth(),
        seed,
        0.25,
        17,
    );
    g.fill(
        bx(BAY[0], BAY[1], 1, U_FLOOR, BAY[4], BAY[5]),
        "minecraft:water",
        None,
    );

    // ---- 3. The east-wall ledge line ---------------------------------------
    for z in 28..=34 {
        let top = ledge_floor(z);
        for x in LEDGE_X..=(LEDGE_X + 1) {
            g.fill(bx(x, x, 0, top, z, z), "minecraft:prismarine_bricks", None);
            g.carve(bx(x, x, top + 1, U_CEIL, z, z));
        }
        g.blk(LEDGE_X + 1, top + 1, z, "minecraft:air", None);
    }
    // a hand-rail of walls on the bay side, so the ledge reads as a ledge
    for z in 28..=34 {
        if z % 2 == 0 {
            g.blk(
                LEDGE_X - 1,
                ledge_floor(z) + 1,
                z,
                "minecraft:prismarine_wall",
                None,
            );
        }
    }

    // ---- 4. Pillar pairs (the TEST ambush) + the item alcove ---------------
    // pairs stand SOUTH of the causeway (z 28..29), so the wardens behind them are
    // hidden from the descent but the walk itself stays open
    for px in [12, 18, 24] {
        for pz in [28] {
            for dx in 0..2 {
                for dz in 0..2 {
                    g.fill(
                        bx(px + dx, px + dx, U_WALK, U_CEIL, pz + dz, pz + dz),
                        "minecraft:mossy_stone_bricks",
                        None,
                    );
                }
            }
        }
    }
    // the alcove the ambush guards: a recess in the north wall
    g.carve(bx(20, 22, U_WALK, U_WALK + 2, U_Z0 - 2, U_Z0 - 1));
    g.fill_pal(
        bx(20, 22, U_FLOOR, U_FLOOR, U_Z0 - 2, U_Z0 - 1),
        &tide_floor(),
        seed,
        0.3,
        19,
    );
    g.blk(
        21,
        U_WALK,
        U_Z0 - 2,
        "minecraft:barrel",
        Some(vec![("facing", "up")]),
    );
    g.blk(
        20,
        U_WALK + 2,
        U_Z0 - 2,
        "minecraft:soul_lantern",
        Some(vec![("hanging", "true")]),
    );

    // ---- 5. The drowned side-cell (the one secret) -------------------------
    // A visibly BROKEN grate is the cue — no illusory wall anywhere in this
    // tileset (dossier §7.3): the way in is seen, not guessed.
    g.carve(bx(20, 23, U_WALK, U_WALK + 2, 36, U_Z1));
    g.fill_pal(
        bx(20, 23, U_FLOOR, U_FLOOR, 36, U_Z1),
        &tide_floor(),
        seed,
        0.3,
        21,
    );
    for x in 20..=23 {
        for y in U_WALK..=(U_WALK + 2) {
            g.blk(x, y, 35, "minecraft:iron_bars", None);
        }
    }
    // the break: two bars gone, low and obvious, with lantern-glow behind them
    g.carve(bx(21, 22, U_WALK, U_WALK + 1, 35, 35));
    g.blk(
        22,
        U_WALK + 2,
        37,
        "minecraft:lantern",
        Some(vec![("hanging", "true")]),
    );
    g.blk(
        21,
        U_WALK,
        U_Z1,
        "minecraft:barrel",
        Some(vec![("facing", "up")]),
    );

    // ---- 6. Upper deck: vestibule, shortcut gallery, far-side landing ------
    for r in [VES, GAL, LND] {
        g.carve(bx(r[0], r[1], KEEP_WALK, UP_CEIL, r[2], r[3]));
        g.fill_pal(
            bx(r[0], r[1], KEEP_FLOOR_Y, KEEP_FLOOR_Y, r[2], r[3]),
            &keep_floor(),
            seed,
            0.2,
            23,
        );
    }
    for z in (VES[2] + 2..=VES[3]).step_by(5) {
        g.blk(
            VES[0],
            KEEP_WALK + 3,
            z,
            "minecraft:lantern",
            Some(vec![("hanging", "false")]),
        );
    }
    for z in (LND[2] + 2..=LND[3]).step_by(5) {
        g.blk(
            LND[1],
            KEEP_WALK + 3,
            z,
            "minecraft:lantern",
            Some(vec![("hanging", "false")]),
        );
    }
    for x in (GAL[0] + 3..=GAL[1]).step_by(6) {
        g.blk(
            x,
            UP_CEIL - 1,
            GAL[2],
            "minecraft:lantern",
            Some(vec![("hanging", "true")]),
        );
    }
    // shortcut door A: sealed from world-load; the near side carries the plate
    // spot the campaign hangs "this door does not open from this side" on.
    g.fill(
        bx(10, 10, KEEP_WALK, KEEP_WALK + 2, 18, 20),
        "minecraft:iron_bars",
        None,
    );
    g.blk(
        9,
        KEEP_WALK + 2,
        19,
        "minecraft:lantern",
        Some(vec![("hanging", "false")]),
    );

    // the drop-ledge overlook: a broken floor bay in the vestibule, kerbed so the
    // commitment is SEEN from the safe side before it is taken.
    g.carve(bx(7, 9, U_CEIL + 1, KEEP_FLOOR_Y, 21, 23));
    for (kx, kz) in [(6, 21), (6, 22), (6, 23), (7, 24), (8, 24), (9, 24)] {
        g.blk(kx, KEEP_WALK, kz, "minecraft:stone_brick_wall", None);
    }

    // ---- 7. The two stairwells ---------------------------------------------
    // envelopes first (see the const block: the hall carve would otherwise have
    // eaten these walls), then the treads.
    g.fill_pal(
        bx(DS_X0 - 1, DS_X1 + 1, U_FLOOR + 1, UP_CEIL, DS_Z0, DS_Z1),
        &tide_wall(),
        seed,
        0.2,
        24,
    );
    g.fill_pal(
        bx(EX_X0 - 1, DUCT_X, U_FLOOR + 1, UP_CEIL, EX_Z0, EX_Z1),
        &tide_wall(),
        seed,
        0.2,
        26,
    );
    for z in DS_Z0..=DS_Z1 {
        let w = descent_walk(z);
        for x in DS_X0..=DS_X1 {
            g.carve(bx(x, x, w, w + 3, z, z));
            if z > DS_Z0 && descent_walk(z - 1) > w {
                stairs(g, x, w - 1, z, "minecraft:stone_brick_stairs", "south");
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
        if z % 3 == 0 {
            g.blk(
                DS_X0,
                descent_walk(z) + 3,
                z,
                "minecraft:soul_lantern",
                Some(vec![("hanging", "false")]),
            );
        }
    }
    for z in EX_Z0..=EX_Z1 {
        let e = exit_walk(z);
        for x in EX_X0..=EX_X1 {
            g.carve(bx(x, x, e, e + 3, z, z));
            if z < EX_Z1 && exit_walk(z + 1) < e {
                stairs(g, x, e - 1, z, "minecraft:stone_brick_stairs", "south");
            } else {
                g.blk(
                    x,
                    e - 1,
                    z,
                    pick(&keep_floor(), value_noise(seed, x, e, z, 0.3, 27)),
                    None,
                );
            }
        }
        if z % 4 == 0 {
            g.blk(
                EX_X1,
                exit_walk(z) + 3,
                z,
                "minecraft:soul_lantern",
                Some(vec![("hanging", "false")]),
            );
        }
    }
    // the stair feet open south into the undercroft hall
    g.carve(bx(DS_X0, DS_X1, U_WALK, U_WALK + 3, DS_Z1, DS_Z1 + 1));
    g.carve(bx(EX_X0, EX_X1, U_WALK, U_WALK + 3, EX_Z1, EX_Z1 + 1));

    // ---- 8. Trap hardware: the dart gallery over the exit stair ------------
    let dw = exit_walk(DART_Z);
    for x in EX_X0..=EX_X1 {
        plate(g, x, dw, DART_Z);
    }
    dispenser(g, DART_DISP, "south");
    // the duct behind the east wall: air where the wire runs and directly above
    // each dust cell, solid everywhere the wire rests on.
    for (cx, cy, cz) in [
        (DUCT_X, 7, 30),
        (DUCT_X, 8, 30),
        (DUCT_X, 8, 29),
        (DUCT_X, 9, 29),
        (DUCT_X, 9, 28),
        (DUCT_X, 10, 28),
    ] {
        g.air(cx, cy, cz);
    }
    wire_dust(
        "tk-cistern",
        g,
        &[[DUCT_X, 7, 30], [DUCT_X, 8, 29], [DUCT_X, 9, 28]],
    );
    // arrow-slit dressing so the gallery reads as a gallery, not one hole
    for z in [26, 32] {
        g.blk(DUCT_X, exit_walk(z) + 2, z, "minecraft:iron_bars", None);
    }
    // the disarm lever's niche: a barred cage in the landing's east corner. It is
    // visible from the whole landing (and so from the stair head the moment the
    // climb ends) but walled off from the walk, entered only round its north jamb
    // — the disarm is the loop's REWARD, never something the stair can reach.
    for y in KEEP_WALK..=(KEEP_WALK + 2) {
        for z in 16..=18 {
            g.blk(37, y, z, "minecraft:iron_bars", None);
        }
        g.blk(38, y, 19, "minecraft:iron_bars", None);
        g.blk(39, y, 19, "minecraft:iron_bars", None);
        g.blk(40, y, 19, "minecraft:iron_bars", None);
    }
    g.blk(
        39,
        KEEP_WALK + 2,
        17,
        "minecraft:soul_lantern",
        Some(vec![("hanging", "true")]),
    );
    g.blk(40, KEEP_WALK, 18, "minecraft:chiseled_stone_bricks", None);

    // ---- 9. Undercroft ambience --------------------------------------------
    for x in 1..SX - 1 {
        for z in U_Z0..=U_Z1 {
            if !g.is_air(x, U_WALK, z) || !g.is_solid(x, U_FLOOR, z) {
                continue;
            }
            let n = hash01(seed, x, U_WALK, z, 31);
            if n < 0.05 && !near_anchor(&anchors(), x, z, 1) {
                g.blk(x, U_WALK, z, "minecraft:moss_carpet", None);
            }
            if g.is_air(x, U_CEIL, z) && hash01(seed, x, U_CEIL, z, 33) < 0.04 {
                g.blk(
                    x,
                    U_CEIL,
                    z,
                    "minecraft:pointed_dripstone",
                    Some(vec![
                        ("vertical_direction", "down"),
                        ("thickness", "tip"),
                        ("waterlogged", "false"),
                    ]),
                );
            }
        }
    }
    // the sparse hanging lamps that keep the hall `dim` rather than lightless
    for (lx, lz) in [
        (6, 28),
        (6, 36),
        (16, 27),
        (28, 27),
        (34, 27),
        (34, 36),
        (24, 37),
    ] {
        g.blk(
            lx,
            U_CEIL,
            lz,
            "minecraft:soul_lantern",
            Some(vec![("hanging", "true")]),
        );
    }

    // Upper deck is a working corridor and is lit properly; the undercroft is
    // lit only enough to stay `dim` (>= the compiler's DARK_THRESHOLD), so the
    // gloom survives without any night-vision grant.
    light_room(
        g,
        VES[0],
        VES[1],
        VES[2],
        VES[3],
        KEEP_WALK,
        UP_CEIL + 1,
        5,
        "minecraft:lantern",
    );
    light_room(
        g,
        GAL[0],
        GAL[1],
        GAL[2],
        GAL[3],
        KEEP_WALK,
        UP_CEIL + 1,
        5,
        "minecraft:lantern",
    );
    light_room(
        g,
        LND[0],
        LND[1],
        LND[2],
        LND[3],
        KEEP_WALK,
        UP_CEIL + 1,
        5,
        "minecraft:lantern",
    );
    let clear_u = |x: i32, z: i32| {
        ((DS_X0 - 1)..=(DS_X1 + 1)).contains(&x) && ((DS_Z0 - 1)..=(DS_Z1 + 1)).contains(&z)
            || ((EX_X0 - 1)..=DUCT_X).contains(&x) && ((EX_Z0 - 1)..=(EX_Z1 + 1)).contains(&z)
    };
    light_room_ex(
        g,
        1,
        SX - 2,
        U_Z0,
        U_Z1,
        U_WALK,
        U_CEIL + 1,
        8,
        "minecraft:soul_lantern",
        &clear_u,
    );
    for x in (3..SX - 3).step_by(7) {
        for z in (U_Z0 + 2..U_Z1).step_by(6) {
            if !clear_u(x, z) {
                chandelier(g, x, z, U_CEIL + 1, 1, "minecraft:lantern");
            }
        }
    }

    // ---- 10. Sockets --------------------------------------------------------
    cut_socket(g, Side::West, KEEP_FLOOR_Y, 19);
    cut_socket(g, Side::East, KEEP_FLOOR_Y, 19);

    // ---- 11. Invariants -----------------------------------------------------
    let mut lr: Vec<[i32; 3]> = Vec::new();
    for z in DS_Z0..=DS_Z1 {
        lr.push([3, descent_walk(z), z]);
    }
    lr.push([3, U_WALK, DS_Z1 + 1]);
    for x in 4..=6 {
        lr.push([x, U_WALK, DS_Z1 + 1]);
    }
    for z in (U_Z0 + 1..=DS_Z1).rev() {
        lr.push([6, U_WALK, z]);
    }
    for x in 7..=LEDGE_X {
        lr.push([x, U_WALK, U_Z0 + 1]);
    }
    for z in 28..=34 {
        lr.push([LEDGE_X, ledge_floor(z) + 1, z]);
    }
    lr.push([LEDGE_X, U_WALK, 35]);
    // the climb is proved on the WEST lane of the stair: the dart gallery's
    // dispenser occupies the head-sweep cell over the east lane, which is exactly
    // the model refusing a rise a player could not make there.
    for x in (LEDGE_X + 1)..=EX_X0 {
        lr.push([x, U_WALK, 35]);
    }
    lr.push([EX_X0, U_WALK, 34]);
    for z in (EX_Z0..=EX_Z1).rev() {
        lr.push([EX_X0, exit_walk(z), z]);
    }
    let long_route = lr;
    assert_route_walkable(
        "tk-cistern",
        "long route (vestibule -> unlock)",
        g,
        &long_route,
    );

    let short: Vec<[i32; 3]> = (VES[1]..=LND[0]).map(|x| [x, KEEP_WALK, 19]).collect();
    // door A is sealed at world-load; the gallery behind it must still be a walk
    // once the campaign's `shortcut` verb opens it.
    let mut probe = Grid::new(g.size);
    std::mem::swap(&mut probe.cells, &mut g.cells);
    probe.carve(bx(10, 10, KEEP_WALK, KEEP_WALK + 2, 18, 20));
    assert_route_walkable("tk-cistern", "short route (post-unlock)", &probe, &short);
    probe.fill(
        bx(10, 10, KEEP_WALK, KEEP_WALK + 2, 18, 20),
        "minecraft:iron_bars",
        None,
    );
    std::mem::swap(&mut probe.cells, &mut g.cells);
    assert!(
        long_route.len() > short.len() * 2,
        "tk-cistern: the long route ({}) must dominate the short one ({}) or the loop does not pay",
        long_route.len(),
        short.len()
    );
}

pub fn anchors() -> Vec<(&'static str, AnchorJson)> {
    let dw = exit_walk(DART_Z);
    vec![
        ("anchor/l3-vestibule", a_pos([4, KEEP_WALK, 19], "east")),
        ("anchor/l3-drop-ledge", a_pos([5, KEEP_WALK, 22], "east")),
        (
            "anchor/l3-commit-gate",
            a_region(
                [DS_X0, KEEP_WALK, DS_Z0],
                [DS_X1, KEEP_WALK + 2, DS_Z0],
                "minecraft:iron_bars",
            ),
        ),
        (
            "anchor/l3-shortcut-a",
            a_region(
                [10, KEEP_WALK, 18],
                [10, KEEP_WALK + 2, 20],
                "minecraft:iron_bars",
            ),
        ),
        (
            "anchor/l3-shortcut-a-sign",
            a_pos([9, KEEP_WALK, 19], "east"),
        ),
        ("anchor/l3-shallows", a_pos([6, U_WALK, 32], "east")),
        ("anchor/l3-ambush-a", a_pos([15, U_WALK, U_Z0 + 3], "north")),
        ("anchor/l3-ambush-b", a_pos([27, U_WALK, U_Z0 + 3], "north")),
        (
            "anchor/l3-item-alcove",
            a_pos([21, U_WALK, U_Z0 - 1], "south"),
        ),
        (
            "anchor/l3-ledge",
            a_pos([LEDGE_X, ledge_floor(31) + 1, 31], "north"),
        ),
        ("anchor/l3-secret", a_pos([22, U_WALK, 37], "north")),
        (
            "anchor/l3-trap-darts",
            a_trap([EX_X0 + 1, dw, DART_Z], "north", DART_DISP, PLATE_BLOCK),
        ),
        ("anchor/l3-dart-lever", a_pos([39, KEEP_WALK, 17], "east")),
        ("anchor/l3-unlock", a_pos([35, KEEP_WALK, 19], "west")),
        ("anchor/l3-landing", a_pos([36, KEEP_WALK, 22], "north")),
    ]
}

pub fn light_regions() -> Vec<[i32; 6]> {
    vec![
        bx(1, SX - 2, U_WALK, U_WALK + 1, U_Z0, U_Z1),
        bx(VES[0], VES[1], KEEP_WALK, KEEP_WALK + 1, VES[2], VES[3]),
        bx(GAL[0], GAL[1], KEEP_WALK, KEEP_WALK + 1, GAL[2], GAL[3]),
        bx(LND[0], LND[1], KEEP_WALK, KEEP_WALK + 1, LND[2], LND[3]),
    ]
}
