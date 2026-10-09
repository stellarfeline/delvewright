//! The world's tests: standability, the step rule, footprints, and the liveness
//! of staged volumes.

use super::*;
use crate::compiler::nav::testkit::*;
use crate::compiler::nav::*;
use crate::compiler::plan::RegionEvents;
use std::collections::{BTreeMap, BTreeSet};

/// A floor at `y - 1` over `[0,w) × [0,d)` with open air to `y + 3`, `solid`
/// extra cells, and one declared furniture region when `region` is `Some`.
fn floored_with_furniture(
    w: i32,
    d: i32,
    y: i32,
    extra: &[[i32; 3]],
    region: Option<([i32; 3], [i32; 3])>,
) -> World {
    let mut solid = BTreeSet::new();
    for x in 0..w {
        for z in 0..d {
            solid.insert([x, y - 1, z]);
        }
    }
    solid.extend(extra.iter().copied());
    World::from_occupancy(
        crate::compiler::assembled::Occupancy {
            solid,
            tall: BTreeSet::new(),
            use_gates: BTreeSet::new(),
            flooded: BTreeSet::new(),
            partial: BTreeMap::new(),
            waterloggable: BTreeSet::new(),
            lava: BTreeSet::new(),
            climb: Default::default(),
            unheld_climb: Default::default(),
        },
        Premises {
            ambient: Ambient::Void,
            base: "void",
            built: Vec::new(),
            lethal_regions: Vec::new(),
            staged_lethal: Vec::new(),
            loop_slabs: Vec::new(),
            furniture_regions: region
                .map(|r| vec![("anchor/table".to_string(), r)])
                .unwrap_or_default(),
            world_load_seals: Vec::new(),
            clocked_gates: BTreeSet::new(),
            objective_cells: Vec::new(),
        },
    )
}

/// **A body may not be proven to stand ON furniture** (spec-0065 §4.1, §9.3).
///
/// One solid block inside a furniture region, an air cell inside the same
/// region beside it, and the floor around both. Over the solid cell: refused.
/// Beside it, feet on the floor: untouched. With its feet in the region's
/// air cell and the floor under it: untouched, because the rule is the
/// support cell's membership, not the body's. The same world with the region
/// removed: all three stand.
#[test]
fn a_body_may_not_stand_on_a_solid_furniture_cell() {
    let top = [3, 65, 3];
    let region = ([3, 65, 3], [4, 65, 3]);
    let on = [3, 66, 3];
    let beside = [2, 65, 3];
    let over_air = [4, 65, 3];
    let fp = Footprint::player();
    let w = floored_with_furniture(8, 8, 65, &[top], Some(region));
    assert!(!w.standable_fp(on, &fp), "the table top is withheld");
    assert!(w.standable_fp(beside, &fp), "the floor beside it is not");
    assert!(
        w.standable_fp(over_air, &fp),
        "an air cell of the region withholds nothing"
    );
    assert_eq!(w.furniture_census(), (1, 1, 1));
    assert_eq!(w.furniture_over(&[on]), vec!["anchor/table"]);
    assert!(w.furniture_over(&[beside, over_air]).is_empty());

    let bare = floored_with_furniture(8, 8, 65, &[top], None);
    for c in [on, beside, over_air] {
        assert!(
            bare.standable_fp(c, &fp),
            "{c:?} stands with no declaration"
        );
    }
    assert_eq!(bare.furniture_census(), (0, 0, 0));
    // …and the counterfactuals lift it.
    assert!(w.without_exclusions().standable_fp(on, &fp));
    assert!(w.without_furniture().standable_fp(on, &fp));
}

/// **A body may not stand on the cell beside a killing volume's face.**
///
/// The impassable set used to be the volume's own cells, so the cell one east
/// of a box was standable, routable and exported — and a walker standing
/// anywhere in it reaches into the box, because a box selector is adjudicated
/// on hitbox intersection. Measured, twice, on the gallery's own ladder: the
/// bot died at `[12, 65, 21]` and `[12, 65, 24]`, both outside the west pit.
#[test]
fn a_walker_may_not_stand_on_the_cell_beside_a_volumes_face() {
    let pit = ([3, 65, 3], [5, 67, 5]);
    let w = floored_with_lethal(12, 12, 65, pit);
    // Inside: refused before and after — this is not what changed.
    assert!(!w.is_standable([4, 65, 4]));
    // The ring. Every cell that shares a face with the box, and the corners.
    for c in [
        [2, 65, 4],
        [6, 65, 4],
        [4, 65, 2],
        [4, 65, 6],
        [2, 65, 2],
        [6, 65, 6],
    ] {
        assert!(
            !w.is_standable(c),
            "a body standing anywhere in {c:?} reaches the box, so it is not footing"
        );
    }
    // …and the refusal stops there. Two cells out is real footing, or the
    // rule would be eating rooms rather than edges.
    assert!(w.is_standable([1, 65, 4]));
    assert!(w.is_standable([7, 65, 4]));
    assert!(w.is_standable([4, 65, 1]));
    // The declared cell count is unchanged: the ring is a routing answer, not
    // a redefinition of what the campaign declared, and `lethal-gate.json`
    // reports the declaration.
    assert_eq!(w.lethal_cells(), 3 * 3 * 3);
    assert!(w.is_lethal([4, 65, 4]));
    assert!(!w.is_lethal([6, 65, 4]));
}

/// A route that only exists by walking the ring is refused, and the refusal
/// can still NAME the volume — `lethal_volumes_over` reads the same widened
/// set, or the author would be sent to look at geometry that was never wrong.
#[test]
fn a_corridor_one_cell_wide_beside_a_volume_is_not_a_way_through() {
    // A 3x12 corridor with the pit filling all but one column of its width.
    let pit = ([0, 65, 4], [1, 67, 6]);
    let w = floored_with_lethal(3, 12, 65, pit);
    assert!(w.is_standable([2, 65, 1]), "the near end is footing");
    assert!(w.is_standable([2, 65, 10]), "so is the far end");
    // The only lane past the pit is x = 2, which is the ring.
    assert!(!w.is_standable([2, 65, 5]));
    assert_eq!(w.find_path([2, 65, 1], [2, 65, 10]), None);
    assert_eq!(
        w.lethal_volumes_over(&[[2, 65, 5]]),
        vec!["lethal/the-pit"],
        "the cell the counterfactual walk would have crossed names the volume"
    );
}

/// Anchor SEATING, not the boundary model ([`AnchorRoot`]). A ceiling anchor —
/// which every spec-0022 `collapse` payload must declare — is two blocks from
/// the cell on top of the roof and three from the floor of its own room, so an
/// unconfined nearest-standable snap seats it on the ROOF, a component no
/// player can walk to. Confining the snap to the declaring piece puts it back
/// on the floor. Both halves are asserted here: the confined seating is the
/// fix, the unconfined one is the defect it replaces.
#[test]
fn a_ceiling_anchor_seats_in_its_room_not_on_the_roof() {
    // A 7×7 room: floor y=63, ceiling y=68, walls between them.
    let mut solid = BTreeSet::new();
    for x in 0..7 {
        for z in 0..7 {
            solid.insert([x, 63, z]);
            solid.insert([x, 68, z]);
            for y in 64..68 {
                if x == 0 || x == 6 || z == 0 || z == 6 {
                    solid.insert([x, y, z]);
                }
            }
        }
    }
    let world = World::from_solid_cells(solid);
    let ceiling_anchor = [3, 67, 3];
    let (floor, roof) = ([3, 64, 3], [3, 69, 3]);

    // The defect: the nearest standable cell by squared distance is the roof
    // (Δy 2) rather than the room's own floor (Δy 3) — a solid ceiling in
    // between counts for nothing.
    let loose = world.reachable_walkable(&[ceiling_anchor]);
    assert!(
        loose.contains(&roof),
        "unconfined snap climbs onto the roof"
    );
    assert!(
        !loose.contains(&floor),
        "and never reaches the room it was declared in"
    );

    // The fix: the piece AABB (y 63..=68) excludes the roof cell entirely.
    let seated = world.reachable_walkable_rooted(&[AnchorRoot {
        at: ceiling_anchor,
        within: ([0, 63, 0], [6, 68, 6]),
    }]);
    assert!(seated.contains(&floor), "confined snap seats in the room");
    assert!(!seated.contains(&roof), "the roof is out of the piece");
}

#[test]
fn snap_finds_floor_in_front_of_a_solid_affordance() {
    // A solid altar block at the target; the nearest standable cell is beside
    // it (the NPC walks up to it, not into it).
    let world = floored(5, 5, 65, &[[2, 65, 2]]);
    assert!(world.snap_standable([2, 65, 2], 0).is_none());
    let snapped = world.snap_standable([2, 65, 2], 2).expect("floor nearby");
    assert!(world.standable(snapped));
    assert!((snapped[0] - 2).abs() + (snapped[2] - 2).abs() <= 1);
}

#[test]
fn snap_none_when_fully_embedded() {
    // A solid cell walled in by solids within the radius → no floor to snap to.
    let mut solid = BTreeSet::new();
    for dx in -2..=2 {
        for dy in -2..=2 {
            for dz in -2..=2 {
                solid.insert([10 + dx, 65 + dy, 10 + dz]);
            }
        }
    }
    let world = World::from_solid_cells(solid);
    assert!(world.snap_standable([10, 65, 10], 2).is_none());
}

#[test]
fn confined_cells_are_standable_distinct_and_ordered_by_distance() {
    // A 5×5 floored room. Placement floods standable cells from the anchor.
    let world = floored(5, 5, 65, &[]);
    let bounds = ([0, 64, 0], [4, 66, 4]);
    let cells = world.confined_standable_cells([2, 65, 2], bounds);
    // Every returned cell is standable, and all are distinct.
    for c in &cells {
        assert!(world.standable(*c), "non-standable cell {c:?}");
    }
    let uniq: BTreeSet<_> = cells.iter().copied().collect();
    assert_eq!(uniq.len(), cells.len(), "duplicate spawn cell");
    // The anchor's own snapped start comes first (distance 0), then its
    // cardinal neighbours (distance 1) before any distance-2 cell.
    assert_eq!(cells[0], [2, 65, 2]);
    // Non-increasing BFS distance is enforced by construction; spot-check that a
    // near cell precedes a far corner.
    let idx = |t: [i32; 3]| cells.iter().position(|c| *c == t).unwrap();
    assert!(idx([2, 65, 3]) < idx([0, 65, 0]));
}

#[test]
fn confined_cells_never_cross_a_socket_seam() {
    // Two 3-wide rooms sharing an open (air) seam at x=3 — as a mated jigsaw
    // socket would be. Confining to the left room's bounds must keep every
    // placement cell at x<=2, never flooding through the open seam into the
    // right room (the den↔mouth spill this fix prevents).
    let mut solid = BTreeSet::new();
    for x in 0..=6 {
        for z in 0..3 {
            solid.insert([x, 64, z]); // continuous floor across both rooms
            solid.insert([x, 67, z]); // ceiling
        }
    }
    let world = World::from_solid_cells(solid);
    let left_bounds = ([0, 64, 0], [2, 66, 2]);
    let cells = world.confined_standable_cells([1, 65, 1], left_bounds);
    assert!(!cells.is_empty());
    for c in &cells {
        assert!(
            c[0] <= 2,
            "placement {c:?} crossed the seam into the right room"
        );
    }
    // Sanity: the floor genuinely connects across the seam (an unconfined flood
    // would reach the right room), so confinement — not a wall — is what holds.
    assert!(world.find_path([1, 65, 1], [5, 65, 1]).is_some());
}

// --- spec-0016 §6: the aggro ring -----------------------------------

#[test]
fn aggro_ring_cells_sit_at_or_just_inside_the_radius() {
    // A 25×25 floored hall; the ring is drawn around its centre at radius 10.
    let world = floored(25, 25, 65, &[]);
    let bounds = ([0, 64, 0], [24, 66, 24]);
    let centre = [12, 65, 12];
    let ring = world.annulus_standable_cells(centre, bounds, 10.0, 1.0);
    assert!(!ring.is_empty(), "an open hall has a ring at radius 10");
    for c in &ring {
        assert!(world.standable(*c), "ring cell {c:?} is standable");
        let d = ((0..3)
            .map(|i| f64::from(c[i] - centre[i]).powi(2))
            .sum::<f64>())
        .sqrt();
        // One-sided on purpose: a cell OUTSIDE follow_range summons a mob
        // that perceives nobody and stands there.
        assert!(
            (9.0..=10.0).contains(&d),
            "ring cell {c:?} at distance {d} is outside [radius-1, radius]"
        );
    }
    // Ordered outermost-first — the edge of perception is where the fiction
    // (and the mechanic) puts them.
    let dist = |c: &[i32; 3]| {
        ((0..3)
            .map(|i| f64::from(c[i] - centre[i]).powi(2))
            .sum::<f64>())
        .sqrt()
    };
    assert!(
        dist(&ring[0]) >= dist(ring.last().unwrap()),
        "the ring is ordered outermost-first: {ring:?}"
    );
    // Deterministic (ADR-0006).
    assert_eq!(
        ring,
        world.annulus_standable_cells(centre, bounds, 10.0, 1.0)
    );
}

#[test]
fn aggro_ring_excludes_cells_that_cannot_see_the_defended_point() {
    // A hall with a full-height wall at x=12 splitting it in two, pierced by
    // nothing: cells on the far side are at ring distance but blind, so a mob
    // summoned there would acquire no target — the mechanic's whole point.
    let mut solid = BTreeSet::new();
    for x in 0..25 {
        for z in 0..9 {
            solid.insert([x, 64, z]);
            solid.insert([x, 68, z]);
        }
    }
    for z in 0..9 {
        for y in 65..=67 {
            solid.insert([18, y, z]);
        }
    }
    // Leave a floor-level gap so the far side stays walk-REACHABLE (this is a
    // sight test, not a reachability test).
    solid.remove(&[18, 65, 4]);
    solid.remove(&[18, 66, 4]);
    let world = World::from_solid_cells(solid);
    let bounds = ([0, 64, 0], [24, 67, 8]);
    let centre = [8, 65, 4];
    let ring = world.annulus_standable_cells(centre, bounds, 10.0, 1.0);
    assert!(!ring.is_empty(), "the near side offers ring cells");
    for c in &ring {
        assert!(
            world.has_line_of_sight(*c, centre),
            "ring cell {c:?} must see the defended point"
        );
    }
    // A far-side cell at exactly ring distance is excluded despite standing
    // and being reachable through the gap.
    let blind = [18 + 1, 65, 0];
    if world.standable(blind) {
        let d = ((0..3)
            .map(|i| f64::from(blind[i] - centre[i]).powi(2))
            .sum::<f64>())
        .sqrt();
        if (9.0..=10.0).contains(&d) {
            assert!(
                !ring.contains(&blind),
                "a blind cell at ring distance must be excluded"
            );
        }
    }
}

#[test]
fn aggro_ring_is_empty_when_the_room_is_smaller_than_the_radius() {
    // The DW0387 shape: a 5×5 room has no cell 20 blocks from its centre.
    let world = floored(5, 5, 65, &[]);
    let bounds = ([0, 64, 0], [4, 66, 4]);
    assert!(
        world
            .annulus_standable_cells([2, 65, 2], bounds, 20.0, 1.0)
            .is_empty()
    );
}

#[test]
fn confined_cells_deterministic_across_runs() {
    let world = floored(6, 4, 65, &[[3, 65, 1]]);
    let bounds = ([0, 64, 0], [5, 66, 3]);
    let a = world.confined_standable_cells([1, 65, 1], bounds);
    let b = world.confined_standable_cells([1, 65, 1], bounds);
    assert_eq!(a, b);
}

#[test]
fn step_up_needs_head_clearance_to_jump() {
    // Lower stand at x=0 (floor y64 → stand y65); raised stand at x=1,2 (floor
    // y65 → stand y66). Reaching the raised floor means jumping up one block at
    // x=0, whose head sweeps the cell two above the feet ([0,67,0]).
    let mk = |low_ceiling: bool| {
        let mut solid = BTreeSet::new();
        solid.insert([0, 64, 0]); // lower floor
        solid.insert([1, 65, 0]); // raised floor
        solid.insert([2, 65, 0]);
        if low_ceiling {
            solid.insert([0, 67, 0]); // ceiling two above the jumper's feet
        }
        World::from_solid_cells(solid)
    };
    // Open headroom: the jump-up is walkable.
    let open = mk(false);
    assert!(open.standable([0, 65, 0]) && open.standable([2, 66, 0]));
    assert!(open.find_path([0, 65, 0], [2, 66, 0]).is_some());
    // A ceiling two above the feet blocks the jump (the entity would head-bonk),
    // so no walkable path exists — the DW0311 case a runtime bot rejects with
    // "No path to the goal!".
    let low = mk(true);
    assert!(low.standable([0, 65, 0]) && low.standable([2, 66, 0]));
    assert!(low.find_path([0, 65, 0], [2, 66, 0]).is_none());
}

#[test]
fn fence_top_is_not_standable_and_fence_is_not_passable() {
    // The owner-hit island bug, modelled: a 1.5-tall oak_fence is neither a
    // floor (no walking player can jump 1.5 onto its top) nor a passable cell.
    let world = classified(3, 1, 65, &[[1, 65, 0]], &[]);
    assert!(
        !world.standable([1, 66, 0]),
        "a fence-top cell must not be standable (the old full-solid model's bug)"
    );
    assert!(
        !world.standable([1, 65, 0]),
        "the fence cell itself must not be passable"
    );
    // The two floor cells beside it are fine but no longer connected.
    assert!(world.standable([0, 65, 0]) && world.standable([2, 65, 0]));
    assert!(
        world.find_path([0, 65, 0], [2, 65, 0]).is_none(),
        "no route through or over a fence line"
    );
}

#[test]
fn open_fence_gate_is_a_passable_threshold_with_no_use_tag() {
    // An authored-open gate (block state open=true) is just a passable cell:
    // no use-gate tag, and even gate-incapable walkers pass it.
    let world = classified(3, 1, 65, &[], &[]); // flat; the "gate" cell is plain air
    assert!(world.find_path([0, 65, 0], [2, 65, 0]).is_some());
    assert!(!world.is_use_gate([1, 65, 0]));
}

#[test]
fn player_footprint_matches_pre_0_6_walkability() {
    // find_path (the delegating wrapper) must equal find_path_fp(player) — the
    // byte-identity guarantee for move-npc / critical-path.
    let world = floored(6, 3, 65, &[[3, 65, 1]]);
    let fp = Footprint::player();
    let a = [0, 65, 1];
    let b = [5, 65, 1];
    assert_eq!(world.find_path(a, b), world.find_path_fp(a, b, &fp));
    // Player footprint is one column, two cells tall.
    assert_eq!(fp.cols, vec![[0, 0]]);
    assert_eq!(fp.height, 2);
}

#[test]
fn tall_footprint_cannot_walk_a_two_high_gap_a_player_fits() {
    // `floored` gives a floor at y-1 and a ceiling at y+2 → two clear cells (y,
    // y+1): a player (2 tall) fits; a warden (2.9 → 3 tall) head-bonks the
    // ceiling, so its footprint has no walkable path (the DW0325 condition).
    let world = floored(6, 3, 65, &[]);
    let a = [0, 65, 1];
    let b = [5, 65, 1];
    let player = Footprint::player();
    let warden = entity_footprint("minecraft:warden");
    assert_eq!(warden.height, 3, "warden is 2.9 tall → 3 cells");
    assert!(
        world.find_path_fp(a, b, &player).is_some(),
        "a player fits the 2-high corridor"
    );
    assert!(
        !world.standable_fp(a, &warden),
        "a warden cannot stand under a 2-high ceiling"
    );
    assert!(
        world.find_path_fp(a, b, &warden).is_none(),
        "a warden cannot walk the 2-high corridor → unroutable"
    );
    // The best-effort blocked-cell reporter names a non-standable cell on the leg.
    let blocked = first_blocked_fp(&world, a, b, &warden);
    assert!(!world.standable_fp(blocked, &warden));
}

#[test]
fn dims_table_and_default_fallback() {
    // Sub-block-wide mobs are single-column; the default fallback is humanoid.
    assert_eq!(entity_footprint("minecraft:sheep").cols, vec![[0, 0]]);
    assert_eq!(entity_footprint("minecraft:sheep").height, 2); // 1.3 → 2
    assert_eq!(entity_footprint("minecraft:iron_golem").height, 3); // 2.7 → 3
    let unknown = entity_footprint("minecraft:some_new_mob");
    assert_eq!(unknown.cols, vec![[0, 0]]);
    assert_eq!(unknown.height, 2);
}

#[test]
fn step_up_from_a_bottom_slab_onto_a_full_block_is_impossible() {
    // THE regression the full-cube model proved wrong. Standing on a bottom
    // slab puts the feet at y=65.5; the neighbouring ledge's top face is at
    // y=67.0 — a **1.5-block** rise, past the ~1.25-block jump apex. The old
    // model saw an ordinary "+1 cell" step (feet cell 66 → 67) and proved a
    // route no player and no mineflayer bot can walk.
    let world = blocks_world(&[
        ([0, 64, 0], "minecraft:stone"), // support under the slab
        ([0, 65, 0], "minecraft:oak_slab[type=bottom]"), // stand at y=65.5
        ([1, 64, 0], "minecraft:stone"),
        ([1, 65, 0], "minecraft:stone"),
        ([1, 66, 0], "minecraft:stone"), // ledge top at y=67.0
    ]);
    // Both standing cells are standable in isolation…
    assert!(world.is_standable([0, 66, 0]), "the slab top is standable");
    assert!(world.is_standable([1, 67, 0]), "the ledge top is standable");
    // …but no step connects them: 1.5 blocks is not jumpable.
    assert!(
        !world.neighbors([0, 66, 0]).contains(&[1, 67, 0]),
        "a 1.5-block rise must not be a legal step: {:?}",
        world.neighbors([0, 66, 0])
    );
    assert!(
        world.find_path([0, 66, 0], [1, 67, 0]).is_none(),
        "no route may cross an unjumpable rise"
    );
    // The same ledge one cell lower IS reachable — a 0.5-block auto-step off
    // the slab. The rule rejects the impossible rise, not the block kind.
    let ok = blocks_world(&[
        ([0, 64, 0], "minecraft:stone"),
        ([0, 65, 0], "minecraft:oak_slab[type=bottom]"),
        ([1, 64, 0], "minecraft:stone"),
        ([1, 65, 0], "minecraft:stone"),
    ]);
    assert!(
        ok.neighbors([0, 66, 0]).contains(&[1, 66, 0]),
        "slab top → full block top is a 0.5 auto-step: {:?}",
        ok.neighbors([0, 66, 0])
    );
}

#[test]
fn step_up_onto_a_bottom_slab_needs_no_jump_headroom() {
    // The other direction — a step vanilla ADMITS that the full-cube model
    // refused. From a full floor onto a bottom slab is a 0.5-block rise: an
    // auto-step (vanilla `maxUpStep` 0.6), not a jump, so a ceiling directly
    // over the walker's jump arc is irrelevant. The old rule treated it as a
    // "+1 cell" jump and demanded head clearance that a real player never
    // needs.
    let world = blocks_world(&[
        ([0, 64, 0], "minecraft:stone"), // stand at y=65
        ([1, 64, 0], "minecraft:stone"),
        ([1, 65, 0], "minecraft:oak_slab[type=bottom]"), // slab top y=65.5
        ([0, 67, 0], "minecraft:stone"),                 // ceiling over the source
    ]);
    assert!(world.is_standable([0, 65, 0]));
    assert!(world.is_standable([1, 66, 0]));
    assert!(
        world.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
        "a 0.5-block auto-step must be legal even under a low ceiling: {:?}",
        world.neighbors([0, 65, 0])
    );
}

#[test]
fn top_slab_and_double_slab_are_full_height_steps() {
    // A `type=top` slab's walkable face IS the cell top, so stepping onto it
    // from a full floor one cell down is an ordinary 1.0-block jump — legal
    // with headroom. The half-step rule must key on the slab HALF, not on the
    // word "slab".
    let world = blocks_world(&[
        ([0, 64, 0], "minecraft:stone"),
        ([1, 64, 0], "minecraft:stone"),
        ([1, 65, 0], "minecraft:oak_slab[type=top]"),
    ]);
    assert!(
        world.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
        "a top slab is a full-height step up: {:?}",
        world.neighbors([0, 65, 0])
    );
    // And from a top slab, the next full block one cell up is a normal 1.0
    // rise — unlike the bottom-slab case above, this stays legal.
    let world = blocks_world(&[
        ([0, 64, 0], "minecraft:stone"),
        ([0, 65, 0], "minecraft:oak_slab[type=top]"),
        ([1, 64, 0], "minecraft:stone"),
        ([1, 65, 0], "minecraft:stone"),
    ]);
    assert!(
        world.neighbors([0, 66, 0]).contains(&[1, 66, 0]),
        "top slab → full block at the same standing cell is level"
    );
}

#[test]
fn snow_layers_step_by_layer_count_and_thin_snow_is_walked_over() {
    // `snow` collision is `(layers-1)*2/16`: one layer has NO collision box at
    // all (walked straight over — the floor is what is under it), five layers
    // is a half-block auto-step, and a deep drift plus a full block above is
    // past the jump apex.
    let thin = blocks_world(&[
        ([0, 64, 0], "minecraft:stone"),
        ([0, 65, 0], "minecraft:snow[layers=1]"),
    ]);
    assert!(
        thin.is_standable([0, 65, 0]),
        "a single snow layer is walked through, not stood on top of"
    );
    let drift = blocks_world(&[
        ([0, 64, 0], "minecraft:stone"),
        ([1, 64, 0], "minecraft:stone"),
        ([1, 65, 0], "minecraft:snow[layers=5]"), // top at +0.5
    ]);
    assert!(
        drift.neighbors([0, 65, 0]).contains(&[1, 66, 0]),
        "a 5-layer drift is a 0.5-block auto-step: {:?}",
        drift.neighbors([0, 65, 0])
    );
    let over = blocks_world(&[
        ([0, 64, 0], "minecraft:stone"),
        ([0, 65, 0], "minecraft:snow[layers=5]"), // stand at y=65.5
        ([1, 64, 0], "minecraft:stone"),
        ([1, 65, 0], "minecraft:stone"),
        ([1, 66, 0], "minecraft:stone"), // ledge top at y=67.0
    ]);
    assert!(
        !over.neighbors([0, 66, 0]).contains(&[1, 67, 0]),
        "1.5 blocks up off a drift is not jumpable: {:?}",
        over.neighbors([0, 66, 0])
    );
}

#[test]
fn walk_cells_visits_every_cell_in_order() {
    // A long axis-aligned run visits each cell once, in order; the first
    // matching cell wins.
    let seen = std::cell::RefCell::new(Vec::new());
    let out = walk_cells([0.5, 0.5, 0.5], [4.5, 0.5, 0.5], |c| {
        seen.borrow_mut().push(c);
        c[0] == 3
    });
    assert_eq!(out, Some([3, 0, 0]));
    assert_eq!(
        *seen.borrow(),
        vec![[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]]
    );
}

/// The cost currency: a flat step is one block of level walking, and every
/// sixteenth of height change (either direction) costs [`ELEV_WEIGHT`] more.
#[test]
fn step_cost_prices_elevation_change_in_walking_distance() {
    let flat = step_cost_16(64 * 16, 64 * 16);
    assert_eq!(
        flat, STEP_COST_16,
        "a level step costs one block of walking"
    );
    // A full block up and the same block down cost alike — bobbing is bobbing.
    let up = step_cost_16(64 * 16, 65 * 16);
    let down = step_cost_16(65 * 16, 64 * 16);
    assert_eq!(up, down);
    assert_eq!(
        up,
        STEP_COST_16 * (1 + ELEV_WEIGHT),
        "a 1-block rise costs one flat step plus ELEV_WEIGHT blocks of detour"
    );
    // A half-block (slab / path lip) costs proportionally less, so intentional
    // slab stairs are not treated like lumpy ground.
    assert!(step_cost_16(64 * 16, 64 * 16 + 8) < up);
    assert!(step_cost_16(64 * 16, 64 * 16 + 8) > flat);
}

/// The weight's own derivation, executable at last.
///
/// `ELEV_WEIGHT`'s doc comment argued from a jump arc of ≈12 airborne ticks
/// against ≈4.6 ticks of flat walking per block, and both of those numbers
/// lived in prose, so the arithmetic could not go red if either moved. They
/// are entries of the metrics table now, and this asserts the relationship
/// rather than executing it: the weight is a TUNED number an owner playtest
/// settled (round 8), so deriving it at run time would let an edit to a
/// physics fact silently move every route in every campaign. Asserting it
/// makes the same edit a red that says which decision has to be re-taken.
#[test]
fn the_elevation_weight_is_the_integer_under_its_jump_arc() {
    use delvewright_dsl::metrics::{JUMP_AIRBORNE_TICKS, walk_ticks_per_block};
    let blocks_of_walking_per_block_of_climb = JUMP_AIRBORNE_TICKS / walk_ticks_per_block();
    assert!(
        (blocks_of_walking_per_block_of_climb - 2.59).abs() < 0.01,
        "a block of climb costs about 2.5 blocks of walking time; got {blocks_of_walking_per_block_of_climb}"
    );
    assert_eq!(
        ELEV_WEIGHT,
        blocks_of_walking_per_block_of_climb.floor() as u32,
        "the weight is deliberately the integer UNDER the physical figure — \
         overpaying for flatness is what would distort routes on legitimately \
         sloped terrain"
    );
}

/// spec-0088 §4.1: **the two readings of one gate**, over synthetic flag
/// writes and a synthetic replay, with the region model's own ancestry rule
/// (a strict predecessor on a linear path, or step 0).
#[cfg(test)]
mod staged_liveness_tests {
    use super::*;
    use crate::compiler::plan::StagedGate;
    use crate::compiler::plan::{DataReplay, FlagEvent};

    fn linear(g: usize, s: usize) -> bool {
        g == 0 || g < s
    }

    fn gate(req: &[&str], forb: &[&str], state: &[(&str, i32)]) -> StagedGate {
        StagedGate {
            requires_flags: req.iter().map(|s| s.to_string()).collect(),
            forbids_flags: forb.iter().map(|s| s.to_string()).collect(),
            requires_state: state
                .iter()
                .map(|(s, v)| delvewright_dsl::StateCompare {
                    state: delvewright_dsl::StateId(s.to_string()),
                    op: delvewright_dsl::CompareOp::AtLeast,
                    value: *v,
                })
                .collect(),
            terms: Vec::new(),
        }
    }

    fn events(flags: &[(&str, usize, bool)]) -> RegionEvents {
        let mut ev = RegionEvents::default();
        ev.flags = flags
            .iter()
            .map(|(f, s, forced)| FlagEvent {
                flag: f.to_string(),
                fire_step: *s,
                forced: *forced,
            })
            .collect();
        ev
    }

    fn at(g: &StagedGate, ev: &RegionEvents, steps: usize) -> Vec<(bool, bool)> {
        (0..steps)
            .map(|s| {
                let l = liveness_of(g, ev, s, &linear);
                (l.may, l.is)
            })
            .collect()
    }

    /// A required flag only a trap payload sets: may be live from step 0, live
    /// on the forced route nowhere.
    #[test]
    fn a_required_flag_only_a_trap_sets_may_live_from_zero_and_is_never_live() {
        let g = gate(&["flag/sprung"], &[], &[]);
        let ev = events(&[("flag/sprung", 0, false)]);
        assert_eq!(at(&g, &ev, 4), vec![(true, false); 4]);
    }

    /// The same flag set by a forced objective at step k: both readings from
    /// the first arrival whose ancestry contains k.
    #[test]
    fn a_required_flag_a_forced_beat_sets_is_live_from_the_next_arrival() {
        let g = gate(&["flag/lid"], &[], &[]);
        let ev = events(&[("flag/lid", 2, true)]);
        assert_eq!(
            at(&g, &ev, 5),
            vec![
                (false, false),
                (false, false),
                (false, false),
                (true, true),
                (true, true)
            ]
        );
    }

    /// A forbidden flag a trap payload sets: may be live unchanged (an
    /// unguaranteed firing may never open), live on the forced route nowhere.
    #[test]
    fn a_forbidden_flag_a_trap_sets_leaves_may_live_and_kills_is_live() {
        let g = gate(&[], &["flag/cold"], &[]);
        let ev = events(&[("flag/cold", 0, false)]);
        assert_eq!(at(&g, &ev, 3), vec![(true, false); 3]);
        // Set only by a forced beat at step 1: live on the forced route until
        // it, dead after it.
        let ev = events(&[("flag/cold", 1, true)]);
        assert_eq!(
            at(&g, &ev, 4),
            vec![(true, true), (true, true), (false, false), (false, false)]
        );
    }

    /// A numeric term the replay decides true at step k: both readings from k;
    /// one an unforced root writes may hold everywhere.
    #[test]
    fn a_numeric_term_the_replay_decides_true_at_k_is_live_from_k() {
        let g = gate(&[], &[], &[("state/water", 3)]);
        let mut ev = RegionEvents::default();
        let vals = |v: i64| {
            [("state/water".to_string(), Some(v))]
                .into_iter()
                .collect::<BTreeMap<String, Option<i64>>>()
        };
        ev.data = DataReplay {
            before: [(1, vals(0)), (2, vals(1)), (3, vals(3))]
                .into_iter()
                .collect(),
            end: vals(3),
            unforced_writers: BTreeSet::new(),
        };
        assert_eq!(
            at(&g, &ev, 5),
            vec![
                (false, false),
                (false, false),
                (false, false),
                (true, true),
                (true, true)
            ]
        );
        ev.data.unforced_writers.insert("state/water".to_string());
        assert!(at(&g, &ev, 3).iter().all(|(may, _)| *may));
        // An undatable value may hold, and is never decided.
        ev.data.unforced_writers.clear();
        ev.data
            .before
            .insert(1, [("state/water".to_string(), None)].into_iter().collect());
        assert_eq!(at(&g, &ev, 2)[1], (true, false));
    }
}
