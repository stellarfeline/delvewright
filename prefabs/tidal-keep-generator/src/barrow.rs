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

/// Ruined dry-stone field courses, `(x0, z0, x1, z1)`, laid across the open
/// pockets between the mounds. Endpoints are indicative only: every cell is
/// filtered by `buildable`, so a course stops where the ground stops taking
/// an obstruction rather than where this table says it ends.
const COURSES: [(i32, i32, i32, i32); 3] = [(10, 22, 17, 22), (28, 20, 36, 20), (12, 6, 19, 6)];

/// Rusted iron stakes, `(x, z, height, lean)`. Fixed and deterministic, and all
/// clear of the promised routes — which `stake` asserts rather than trusts.
const STAKES: [(i32, i32, i32, (i32, i32)); 6] = [
    (15, 30, 3, (1, 0)),
    (34, 30, 4, (-1, 0)),
    (11, 18, 3, (0, 1)),
    (30, 25, 4, (1, 0)),
    // the crossbar marker at the head of the strand — straight, so the bar sits
    // square on the post rather than beside a leaning top
    (20, 8, 4, (0, 0)),
    (37, 15, 3, (0, -1)),
];

/// How far the rock headland reaches in from each landward edge. The north is
/// shallower than the east because the flank lanes cross the piece at z=4 and
/// the headland may not stand on them — `assert_headland_clear` is what says so
/// out loud, and what caught this table's first values.
const CLIFF_DEPTH_E: i32 = 5;
/// Fixed stream for the headland's broken profile: the bound of the box is a
/// property of the piece, not of the seed it is dressed with.
const CLIFF_SALT: u64 = 0x7B17_C11F;
const CLIFF_DEPTH_N: i32 = 3;
/// West of this the shore is open water. The seaward side has no bound because
/// it needs none.
const SEA_X: i32 = 2;

/// Top solid y of the ground at (x,z) before the mounds are raised. `None` means
/// open sea: seabed solid at y=0, water at y=1..2.
///
/// A box-garden still may not let a walkable cell border the void, but that
/// obligation says nothing about what the bound should LOOK like, and the first
/// cut answered it with a two-cell lip at chest height around three sides — a
/// kerb, which is the one thing a box garden must never read as. The reference
/// image bounds this shore two different ways and neither is a wall: **rock
/// that rises** on the landward sides, receding into mist, and **open water**
/// seaward, which stops a player without any geometry saying so. So the west
/// runs into the sea, and the north and east climb 11 blocks over five cells —
/// too tall to read as an edge, which is exactly why it stops reading as one.
fn surface(x: i32, z: i32) -> Option<i32> {
    if z > TIDE_Z || x < SEA_X {
        return None;
    }
    // the gate approach keeps its cleft through the north headland
    let socket_lane = (x - LINE_X).abs() <= 2;
    let east = x - (SX - 1 - CLIFF_DEPTH_E);
    let north = CLIFF_DEPTH_N - z;
    let inset = if socket_lane { east } else { east.max(north) };
    if inset > 0 {
        // Broken, not stepped. `inset * 2 + 1` alone is an arithmetic ramp and
        // renders as a flight of stairs — the second way to say "box wall",
        // after the kerb. The jitter is the same value-noise field the palettes
        // use, on a fixed salt so the face is part of the piece's determinism
        // and not of its seed.
        let jitter = (value_noise(CLIFF_SALT, x, 0, z, 0.26, 61) * 7.0).round() as i32;
        let top = SHORE_FLOOR_Y + inset * 3 + jitter - 2;
        return Some(top.clamp(SHORE_FLOOR_Y + 2, SY - 1));
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

/// Bare walk-plane ground: not a mound, not standing in a pool, not already
/// occupied, and clear of the anchor cells themselves.
fn bare_ground(g: &Grid, x: i32, z: i32) -> bool {
    g.is_air(x, SHORE_WALK, z)
        && mound_height(x, z) == 0
        && g.name_at(x, SHORE_FLOOR_Y, z) != Some("minecraft:water")
        && !near_anchor(&anchors(), x, z, 1)
}

/// Where dressing may STAND. Heaps, boulders and wall courses are obstructions,
/// so they keep off every promised route and off the anchors with margin.
fn buildable(g: &Grid, x: i32, z: i32) -> bool {
    bare_ground(g, x, z) && !on_route(x, z) && !near_anchor(&anchors(), x, z, 2)
}

/// Where dressing may be PAINTED. A wrack mat is a carpet: one sixteenth of a
/// block, no collision, no step, nothing a walker can be stopped by.
///
/// The route exclusion exists to keep the bypass ground free of OBSTRUCTIONS,
/// and it must not be spent on things that obstruct nothing. Applying it to
/// mats deleted the wrack line outright — the flank traverse runs the full
/// width of the piece at z=32, so `on_route` blankets the entire tide band,
/// which is precisely where the tide banks its wrack and where the reference
/// image is darkest. The dressing looked scattered inland because the one place
/// it belonged was the one place it was forbidden.
fn paintable(g: &Grid, x: i32, z: i32) -> bool {
    bare_ground(g, x, z)
}

/// Wrack lies in clumps, not in speckle, so WHERE it falls is drawn from the
/// smooth noise field rather than the per-cell hash. An even scatter at the same
/// density reads as ground texture; the reference image's kelp reads as *mass*,
/// and that difference is the whole signature of the zone.
fn wrack_field(seed: u64, x: i32, y: i32, z: i32) -> f64 {
    value_noise(seed, x, y, z, 0.30, 37)
}

/// WHICH wrack block, on its own stream. Not `wrack_field`: that value has
/// already been through a `> t` test at every call site, so feeding it to
/// `pick` samples the top tail of the distribution and the palette collapses to
/// its last entry. The first cut of this shipped 189 `black_wool` and 8
/// `sculk` from a 62/38 mix — a bug no eye catches and the block census names
/// on sight.
fn wrack_pick(seed: u64, x: i32, y: i32, z: i32) -> &'static str {
    pick(&wrack(), value_noise(seed, x, y, z, 0.55, 39))
}

/// How much wrack a cell is entitled to, 0..1.
///
/// The tide banks its wrack where it last stood, and against whatever stopped
/// it — the open flat between is bare. The first cut scattered wrack over the
/// whole field on one flat threshold and rendered as a patchwork: every part of
/// the shore equally dressed is every part of the shore equally unread. This is
/// the structure the reference image actually has, and it is why the middle of
/// the field is now empty enough for the elite to be the thing you look at.
fn wrack_bias(x: i32, z: i32) -> f64 {
    let inland = (TIDE_Z - z).max(0) as f64;
    let tide = (1.0 - inland / 13.0).clamp(0.0, 1.0);
    let against_mound = BARROWS
        .iter()
        .map(|&(cx, cz, rad)| {
            let d = ((((x - cx).pow(2) + (z - cz).pow(2)) as f64).sqrt()) - rad as f64;
            (1.0 - d / 4.0).clamp(0.0, 1.0)
        })
        .fold(0.0f64, f64::max);
    tide.max(against_mound * 0.9)
}

/// The rusted iron stakes of the reference image: the only warm colour anywhere
/// in the frame, which is what carries the eye across an otherwise fully
/// desaturated field.
///
/// **Waxed** copper bars, and the wax is not decoration. Bare copper oxidises on
/// a live server, so an unwaxed stake would be brown on the day the delve ships
/// and green some hours into a playthrough — a prefab whose appearance drifts
/// under the player. The waxed variant is fixed at the exposed stage forever.
///
/// Bars rather than `lightning_rod` for the same reason the palettes are
/// measured: the rod's texture is (196,111,83), a bright signal orange that
/// renders as a lit torch, where `exposed_copper_bars` is (134,107,89) — the
/// muted rust the image actually carries. Bars also connect to their
/// neighbours, so a lean is a real bend rather than a floating segment: the
/// shaft rises, steps one cell through a joint, and finishes offset.
const STAKE_BLOCK: &str = "minecraft:waxed_exposed_copper_bars";

/// The assert is the load-bearing part. A stake is a 3–4 block obstruction, and
/// the piece's whole promise is that the elite can be walked past on either
/// flank; naming the offending cell here is worth more than letting
/// `assert_route_walkable` report "route blocked" from three steps away.
fn stake(g: &mut Grid, x: i32, z: i32, h: i32, lean: (i32, i32)) {
    assert!(
        !on_route(x, z),
        "tk-barrow-field: stake at ({x},{z}) stands on a promised route — the shore's \
         dressing may not narrow the ground the optional-elite bypass is proved on"
    );
    let straight = lean == (0, 0);
    let shaft = if straight { h } else { h - 1 };
    for k in 0..shaft {
        g.blk(x, SHORE_WALK + k, z, STAKE_BLOCK, None);
    }
    if !straight {
        // the head kicks over one cell; the joint below it is what the bars
        // connect through, so nothing is left hanging in air
        g.blk(
            x + lean.0,
            SHORE_WALK + shaft - 1,
            z + lean.1,
            STAKE_BLOCK,
            None,
        );
        g.blk(
            x + lean.0,
            SHORE_WALK + shaft,
            z + lean.1,
            STAKE_BLOCK,
            None,
        );
    }
}

pub fn build(g: &mut Grid, seed: u64) {
    // 1. Ground: solid plinth up to the surface, silt at the tide, wet shingle
    //    inland. Nothing on this shore is sand and nothing on it is grass — see
    //    `tidal_flat()` for where that comes from.
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
                                pick(&tide_silt(), value_noise(seed, x, y, z, 0.18, 13))
                            } else if mound_height(x, z) > 0 {
                                pick(&barrow_stone(), value_noise(seed, x, y, z, 0.3, 15))
                            } else if top > SHORE_FLOOR_Y {
                                pick(&plinth(), value_noise(seed, x, y, z, 0.16, 17))
                            } else {
                                pick(&tidal_flat(), value_noise(seed, x, y, z, 0.15, 19))
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

    // 1b. Tide pools: the sheet water the reference image leaves standing on the
    //     flat once the tide is out — the thing that makes the ground read as
    //     WET rather than merely grey. Exactly one cell deep, cut into the walk
    //     datum with the plinth already solid beneath, so they cost nothing in
    //     walkability and a player can step straight through them. They sit at
    //     the piece's declared `waterline_y`, which is where standing water on a
    //     tidal flat belongs.
    for x in 3..SX - 3 {
        for z in 3..TIDE_Z {
            if surface(x, z) != Some(SHORE_FLOOR_Y) || mound_height(x, z) > 0 {
                continue;
            }
            if on_route(x, z) || near_anchor(&anchors(), x, z, 2) {
                continue;
            }
            // 0.68, not the 0.80 the seagrass scatter above uses. `value_noise`
            // is a trilinear blend of eight hashes, so it is bell-shaped about
            // 0.5, not uniform: its far tail is worth a fraction of a percent
            // and a threshold read as "the top fifth" silently means "almost
            // nothing". Every constant here is calibrated against a measured
            // cell count, never against how the number reads.
            if value_noise(seed, x, SHORE_FLOOR_Y, z, 0.22, 41) > 0.68 {
                g.blk(x, SHORE_FLOOR_Y - 1, z, "minecraft:clay", None);
                g.blk(x, SHORE_FLOOR_Y, z, "minecraft:water", None);
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

    // 2b. Wrack over the cairns. In the reference image the mounds are not bare
    //     stone — they are half-buried under black kelp, and that contrast (pale
    //     lichened cobble under wet black mass) is what makes them read as
    //     cairns at the far end of the field rather than as rubble piles. Drawn
    //     from the clumping field so the drapes are masses, not stipple.
    for (cx, cz, rad) in BARROWS {
        for dx in -rad..=rad {
            for dz in -rad..=rad {
                let (x, z) = (cx + dx, cz + dz);
                let h = mound_height(x, z);
                if h == 0 || !g.inb(x, SHORE_FLOOR_Y + h + 1, z) {
                    continue;
                }
                let top = SHORE_FLOOR_Y + h;
                if !g.is_air(x, top + 1, z) {
                    continue;
                }
                let w = wrack_field(seed, x, top, z);
                if w > 0.69 {
                    g.blk(x, top + 1, z, wrack_pick(seed, x, top, z), None);
                }
            }
        }
    }

    // 3. Shore dressing: the wrack line, seagrass in the shallows, and the
    //    stones the tide has left out on the flat. Never on a route cell
    //    (proved in step 7).
    //
    //    Kelp is this zone's dominant visual mass in the reference image, and
    //    it comes in two forms because it does in the image: sodden mats lying
    //    flat on the silt, and the heaps the tide banks against anything that
    //    stopped it. The two forms answer to different rules — a mat paints
    //    over any bare ground including a route, a heap is an obstruction and
    //    keeps off — which is the whole reason `paintable` and `buildable` are
    //    separate. Nothing here is a plant: there is no grass and no scrub
    //    anywhere on this shore, which is why `short_grass` and `dead_bush` are
    //    gone rather than merely thinned.
    for x in 2..SX - 2 {
        for z in 2..SZ {
            if surface(x, z).is_none() {
                if value_noise(seed, x, 1, z, 0.4, 31) > 0.80 {
                    g.blk(x, 1, z, "minecraft:seagrass", None);
                }
                continue;
            }
            if !paintable(&*g, x, z) {
                continue;
            }
            let w = wrack_field(seed, x, SHORE_WALK, z) + 0.34 * wrack_bias(x, z);
            let mat = |g: &mut Grid| {
                g.blk(x, SHORE_WALK, z, "minecraft:black_carpet", None);
            };
            if w > 0.93 && buildable(&*g, x, z) {
                // a boulder the tide left standing, kelp banked over it
                g.blk(
                    x,
                    SHORE_WALK,
                    z,
                    pick(
                        &barrow_stone(),
                        value_noise(seed, x, SHORE_WALK, z, 0.3, 35),
                    ),
                    None,
                );
                g.blk(
                    x,
                    SHORE_WALK + 1,
                    z,
                    wrack_pick(seed, x, SHORE_WALK + 1, z),
                    None,
                );
            } else if w > 0.81 {
                mat(g);
            }
        }
    }

    // 3b. Broken dry-stone courses: the field walls of the reference image, all
    //     ruined down to one or two courses and gapped. Dressing, not
    //     enclosure — every cell is gated on the same `buildable` test as the
    //     wrack, so a course can be eaten by a mound or stop dead at a route
    //     without ever crossing one.
    for (x0, z0, x1, z1) in COURSES {
        let n = (x1 - x0).abs().max((z1 - z0).abs());
        for i in 0..=n {
            let x = x0 + (x1 - x0).signum() * i;
            let z = z0 + (z1 - z0).signum() * i;
            if !buildable(&*g, x, z) || hash01(seed, x, SHORE_WALK, z, 51) < 0.22 {
                continue;
            }
            g.blk(
                x,
                SHORE_WALK,
                z,
                pick(
                    &barrow_stone(),
                    value_noise(seed, x, SHORE_WALK, z, 0.3, 53),
                ),
                None,
            );
            if hash01(seed, x, SHORE_WALK, z, 55) < 0.45 {
                g.blk(x, SHORE_WALK + 1, z, "minecraft:cobblestone_wall", None);
            }
        }
    }
    // driftwood spars above the tide line (two fixed, deterministic). The west
    // 3c. The stakes. There is no wood anywhere in the reference image — the
    //     things standing out of this shore are rusted iron, leaning where the
    //     tide has worked at their footings. They are also the only warm colour
    //     in the frame, so they double as the shore's depth cue: a player
    //     reading the field from the landing has six warm marks receding into a
    //     grey flat.
    for (sx, sz, h, lean) in STAKES {
        stake(g, sx, sz, h, lean);
    }
    // One stake carries a crossbar — the marker the image plants at the head of
    // the strand. Same object, one bar, so it reads as deliberate among five
    // that are merely leaning. Bars connect to their neighbours on their own,
    // so the arms need no state: placing them beside the shaft IS the joint.
    g.blk(21, SHORE_WALK + 2, 8, STAKE_BLOCK, None);
    g.blk(19, SHORE_WALK + 2, 8, STAKE_BLOCK, None);

    // 4. BF1 "Barrow Fire": a lit campfire in a stone ring, its rest cell SOUTH
    //    of the flame so the anchor stays standable (`DW0316`) and the party
    //    rests with the field — and the kneeling elite — in front of them.
    //
    //    Placement is a `DW0478` constraint, not taste (owner ruling 2026-08-04,
    //    task #132): a bonfire may not stand inside any hostile's aggro range,
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
    // the seat beside the fire is a tide-worn boulder, not a log: this shore has
    // no wood on it anywhere, and a driftwood bench was the one place the old
    // palette let some in.
    g.blk(10, SHORE_WALK, 28, "minecraft:mossy_cobblestone", None);

    // 5. The elite's dressing: a leaning banner pole beside the barrow it sleeps
    //    against — the conspicuous over-dress that reads "this one is different"
    //    from the far end of the field (dossier §3.3 signal 4).
    //    The pole is one of the shore's own iron stakes, taller than the six
    //    standing out on the flat: the over-dress is that somebody hung a
    //    standard on a grave marker, which is legible from distance precisely
    //    because the player has already learned what a bare stake looks like.
    for y in SHORE_WALK..=(SHORE_WALK + 3) {
        g.blk(23, y, 14, STAKE_BLOCK, None);
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
    assert_headland_clear();
    for (label, route) in routes() {
        assert_route_walkable("tk-barrow-field", label, g, &route);
    }
}

/// The bypass promise, proved at generation time: no burial mound may reach the
/// centre corridor or either flank lane. An "optional" elite whose flank ground
/// is blocked is not optional — and the compiler's optional-elite bypass proof
/// would be arguing about geometry this piece got wrong.
fn assert_headland_clear() {
    for (label, route) in routes() {
        for c in route {
            assert!(
                surface(c[0], c[2]) == Some(SHORE_FLOOR_Y),
                "tk-barrow-field: the headland stands on route `{label}` at ({},{}) — the \
                 shore's bound may rise anywhere the player does not have to walk, and \
                 nowhere they do",
                c[0],
                c[2]
            );
        }
    }
}

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
