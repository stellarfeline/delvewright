//! Synthetic worlds and fixtures the nav tests of more than one object share.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// A flat solid floor at `y-1` over `[0,w) × [0,d)`, with the given interior
/// cells at `y` set solid (obstacles). Cells at `y` not listed are open air.
pub(in crate::compiler::nav) fn floored(w: i32, d: i32, y: i32, walls: &[[i32; 3]]) -> World {
    let mut solid = BTreeSet::new();
    for x in 0..w {
        for z in 0..d {
            solid.insert([x, y - 1, z]); // floor
            solid.insert([x, y + 2, z]); // ceiling (headroom = y, y+1)
        }
    }
    for &c in walls {
        solid.insert(c);
    }
    World::from_solid_cells(solid)
}

/// [`floored`], plus one declared lethal volume — the geometry the
/// footprint-versus-volume rule is stated over.
pub(in crate::compiler::nav) fn floored_with_lethal(
    w: i32,
    d: i32,
    y: i32,
    region: ([i32; 3], [i32; 3]),
) -> World {
    let mut solid = BTreeSet::new();
    for x in 0..w {
        for z in 0..d {
            solid.insert([x, y - 1, z]);
            solid.insert([x, y + 2, z]);
        }
    }
    World::from_occupancy(
        crate::compiler::assembled::Occupancy {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
            // This world has no ambient water, so nothing reads it.
            waterloggable: BTreeSet::new(),
            lava: BTreeSet::new(),
        },
        // The premises spelled out rather than derived from
        // `Premises::geometry_only()`: a struct literal is what makes a
        // premise added later break THIS helper's compilation, which is the
        // visibility the type exists for.
        Premises {
            ambient: Ambient::Void,
            // A synthetic world declares nothing, so it is what a campaign
            // writing no horizon gets — the reading `geometry_only` states.
            base: "void",
            built: Vec::new(),
            lethal_regions: vec![("lethal/the-pit".to_string(), region)],
            staged_lethal: Vec::new(),
            loop_slabs: Vec::new(),
            furniture_regions: Vec::new(),
            world_load_seals: Vec::new(),
            clocked_gates: BTreeSet::new(),
            objective_cells: Vec::new(),
        },
    )
}

/// The linear "every earlier step is an ancestor" gate-ordering used by the
/// synthetic gate tests (no parallel branches). Production routing uses the
/// campaign's real DAG-causal predicate (`Plan::gate_fired_before`).
pub(in crate::compiler::nav) fn linear(g: usize, s: usize) -> bool {
    g < s
}

/// One boundary-proof root whose declaring "piece" is the whole synthetic
/// world. These fixtures build a single free-standing shape a few blocks
/// across and test the void/ocean MODEL, so the piece AABB is that shape —
/// stated explicitly rather than left open, because an unbounded root is the
/// exact defect [`AnchorRoot`] exists to prevent and no production path may
/// construct one.
pub(in crate::compiler::nav) fn roots(at: [i32; 3]) -> [AnchorRoot; 1] {
    [AnchorRoot {
        at,
        within: ([-64, 0, -64], [64, 128, 64]),
    }]
}

/// The plate's own AABB — one "piece" covering the built cells and nothing
/// beyond them.
pub(in crate::compiler::nav) fn plate_built() -> Vec<(String, Bbox)> {
    vec![("prefab/plate".to_string(), ([0, 63, 0], [2, 64, 2]))]
}

/// The `ocean` horizon's ambient: sea level 62, sea floor 54 (the pinned
/// superflat `crate::compiler::plan::SEA_LEVEL` / `SEA_FLOOR_TOP_Y`), with `covered`
/// standing in for the placed pieces' AABBs.
pub(in crate::compiler::nav) fn ocean(
    solid: BTreeSet<[i32; 3]>,
    flooded: BTreeSet<[i32; 3]>,
    covered: Vec<Bbox>,
) -> World {
    World::from_solid_and_flooded(solid, flooded).with_ambient(
        Ambient::Ocean(Sea {
            level: 62,
            floor_top: 54,
        }),
        built(covered),
    )
}

/// A built volume from bare AABBs, naming each box after its index — a
/// synthetic stand-in for the prefab ids [`built_volume`] reads off a plan.
pub(in crate::compiler::nav) fn built(boxes: Vec<Bbox>) -> Vec<(String, Bbox)> {
    boxes
        .into_iter()
        .enumerate()
        .map(|(i, b)| (format!("piece-{i}"), b))
        .collect()
}

/// An inclusive world AABB, as [`World::built`] carries it.
pub(in crate::compiler::nav) type Bbox = ([i32; 3], [i32; 3]);

/// A visited critical position at a given step (spec-0016 §7 lints select on
/// `src_step`, which the plain `vp` helper always leaves at 0).
pub(in crate::compiler::nav) fn vp_at(pos: [i32; 3], src_step: usize) -> VisitedPos {
    VisitedPos {
        pos,
        transport_before: false,
        talk_to: false,
        src_step,
        by_link: false,
        by_loop: false,
    }
}

/// Whether an entity of `width` standing (feet) at `p` has any part of its AABB
/// inside a solid cell. Height 1.95 (the player/villager box).
pub(in crate::compiler::nav) fn aabb_clips(world: &World, p: [f64; 3], width: f64) -> bool {
    aabb_clips_body(world, p, width, 1.95)
}

/// [`aabb_clips`] for a body of the given hitbox. The height became an argument
/// with isl-58: a waypoint is now allowed off the cell floor (the hop apex), so
/// which courses the body occupies stops being a constant.
pub(in crate::compiler::nav) fn aabb_clips_body(
    world: &World,
    p: [f64; 3],
    width: f64,
    height: f64,
) -> bool {
    let span = |c: f64| {
        (
            (c - width / 2.0).floor() as i32,
            (c + width / 2.0 - 1e-9).floor() as i32,
        )
    };
    let (x0, x1) = span(p[0]);
    let (z0, z1) = span(p[2]);
    let (y0, y1) = (p[1].floor() as i32, (p[1] + height - 1e-9).floor() as i32);
    (x0..=x1).any(|x| (z0..=z1).any(|z| (y0..=y1).any(|y| world.solid_at([x, y, z]))))
}

// --- collision-accurate standability: fences / walls / fence gates ---

/// A world from explicit collision classes: a flat solid floor at
/// `y-1` over `[0,w) × [0,d)` with the given `tall` (fence/wall) and
/// `use_gates` (closed fence gate) cells at stand level.
pub(in crate::compiler::nav) fn classified(
    w: i32,
    d: i32,
    y: i32,
    tall: &[[i32; 3]],
    use_gates: &[[i32; 3]],
) -> World {
    let mut solid = BTreeSet::new();
    for x in 0..w {
        for z in 0..d {
            solid.insert([x, y - 1, z]);
        }
    }
    World::from_occupancy(
        crate::compiler::assembled::Occupancy {
            solid,
            tall: tall.iter().copied().collect(),
            use_gates: use_gates.iter().copied().collect(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
            waterloggable: BTreeSet::new(),
            lava: BTreeSet::new(),
        },
        Premises::geometry_only(),
    )
}

// --- v0.6 checkpoint / stealth proofs (spec-0012 / spec-0014) ---

/// Two floor patches (x∈{0,1} and x∈{3,4}) with a void gap at x=2.
pub(in crate::compiler::nav) fn split_world(y: i32) -> World {
    let mut solid = BTreeSet::new();
    for x in [0, 1, 3, 4] {
        for z in 0..3 {
            solid.insert([x, y - 1, z]); // floor
            solid.insert([x, y + 2, z]); // ceiling
        }
    }
    World::from_solid_cells(solid)
}

pub(in crate::compiler::nav) fn at_step(pos: [i32; 3], src_step: usize) -> VisitedPos {
    VisitedPos {
        pos,
        transport_before: false,
        talk_to: false,
        src_step,
        by_link: false,
        by_loop: false,
    }
}

// --- v0.6 trap completability proof (spec-0011, DW0342) ---

/// A 1-wide walkable corridor along z=1: a floor strip at `[0..len, y-1, 1]`.
/// `[x, y, 1]` are the only standable cells, so the corridor has no bypass — a
/// cell on it is a genuine chokepoint the player cannot walk around.
pub(in crate::compiler::nav) fn corridor(len: i32, y: i32) -> World {
    let mut solid = BTreeSet::new();
    for x in 0..len {
        solid.insert([x, y - 1, 1]);
    }
    World::from_solid_cells(solid)
}

// --- hazard observability (spec-0016 §4 addendum, DW0388) ---

/// The y every observability fixture walks on. Feet at `WY`, head at `WY + 1`.
pub(in crate::compiler::nav) const WY: i32 = 65;

/// A synthetic world stated in terms of what is **open**: `open` lists the
/// `(x, z)` columns a player can stand in at [`WY`]. Everything else inside the
/// padded bounding box is solid rock at feet and head height, with a floor
/// below and a lid above. Sightlines are the whole subject here, so describing
/// the carved space directly is what makes each fixture's geometry readable.
pub(in crate::compiler::nav) fn carved(open: &[[i32; 2]]) -> World {
    let air: BTreeSet<[i32; 2]> = open.iter().copied().collect();
    let xs: Vec<i32> = open.iter().map(|c| c[0]).collect();
    let zs: Vec<i32> = open.iter().map(|c| c[1]).collect();
    let (x0, x1) = (xs.iter().min().unwrap() - 3, xs.iter().max().unwrap() + 3);
    let (z0, z1) = (zs.iter().min().unwrap() - 3, zs.iter().max().unwrap() + 3);
    let mut solid = BTreeSet::new();
    for x in x0..=x1 {
        for z in z0..=z1 {
            solid.insert([x, WY - 1, z]); // floor
            solid.insert([x, WY + 2, z]); // lid
            if !air.contains(&[x, z]) {
                solid.insert([x, WY, z]);
                solid.insert([x, WY + 1, z]);
            }
        }
    }
    World::from_solid_cells(solid)
}

/// A one-wide run of open columns along z at a fixed x.
pub(in crate::compiler::nav) fn run_z(x: i32, z0: i32, z1: i32) -> Vec<[i32; 2]> {
    (z0..=z1).map(|z| [x, z]).collect()
}

/// A synthetic shortcut-door world (spec-0016 §2): a room `w × d` split by a
/// solid wall at `z = zw`, with a 1-cell **gate** doorway at `x = gx` and an
/// optional **bypass** hole at `x = bx` (the long way round). The gate cells
/// are open in the base world — the assembled model always clears a gate
/// region — and the proof re-seals them itself.
pub(in crate::compiler::nav) fn shortcut_world(
    w: i32,
    d: i32,
    y: i32,
    zw: i32,
    gx: i32,
    bypass: Option<i32>,
) -> World {
    let mut walls = Vec::new();
    for x in 0..w {
        if x == gx || Some(x) == bypass {
            continue;
        }
        walls.push([x, y, zw]);
        walls.push([x, y + 1, zw]);
    }
    floored(w, d, y, &walls)
}

// --- partial floor heights: a physical step rule ---------------

/// A world from an explicit cell→block map, through the real classifier — the
/// only way to exercise the partial-height model end to end.
pub(in crate::compiler::nav) fn blocks_world(cells: &[([i32; 3], &str)]) -> World {
    let map: BTreeMap<[i32; 3], String> =
        cells.iter().map(|(c, n)| (*c, (*n).to_string())).collect();
    World::from_occupancy(
        crate::compiler::assembled::occupancy_of(map, &BTreeSet::new()),
        Premises::geometry_only(),
    )
}

// --- causally-sealed waypoint export + trap forcing ------------

/// A 5-long, 3-wide room at y=65 whose only two lanes (z=0 and z=2) run from
/// x=0 to x=4; the middle lane z=1 is walled. Sealing one lane's chokepoint
/// forces the route onto the other.
pub(in crate::compiler::nav) fn two_lane_room(y: i32) -> World {
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..5 {
        for z in 0..3 {
            cells.push(([x, y - 1, z], "minecraft:stone"));
        }
        // The middle lane is a solid wall at stand + head height.
        cells.push(([x, y, 1], "minecraft:stone"));
        cells.push(([x, y + 1, 1], "minecraft:stone"));
    }
    blocks_world(&cells)
}
