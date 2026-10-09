//! The router's tests: legs, exports, and the shaped step cost.

use super::*;
use crate::compiler::nav::testkit::*;
use crate::compiler::nav::*;
use crate::compiler::plan::RegionEvents;
use crate::compiler::plan::{RegionEvent, RegionWrite, Step};
use delvewright_dsl::metrics::PLAYER_WIDTH;
use std::collections::{BTreeMap, BTreeSet};

/// **A press from outside the volume is found** (spec-0092 §10,
/// `DW0932`'s "pressed from outside its volume"): a floor of stone, a lever
/// body at x 3, a volume over x 0..=2. Every standable cell in the walk region
/// within a strike of the body and outside the volume is a press cell; a cell
/// out of the walk region, or beyond a strike, is not.
#[test]
fn a_press_from_outside_the_volume_is_found() {
    let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
    for x in 0..12 {
        blocks.insert([x, 63, 0], "minecraft:stone".to_string());
    }
    let occ = crate::compiler::assembled::occupancy_of(blocks, &BTreeSet::new());
    let w = World::from_occupancy(occ, Premises::geometry_only());
    let mut link = crate::compiler::link::LinkPlan {
        trigger_id: "trigger/t".to_string(),
        on: "use",
        anchor_id: Some("anchor/a".to_string()),
        npc_id: None,
        assembly_id: None,
        range: None,
        body: vec![[3, 64, 0]],
        path: "/content/triggers/0/effects/0".to_string(),
        from_anchor: "anchor/deck".to_string(),
        from: ([0, 64, 0], [2, 65, 0]),
        from_area: "area/a".to_string(),
        to_anchor: "anchor/landing".to_string(),
        to_area: "area/a".to_string(),
        to: [10, 64, 0],
        tick: 0,
        requires_flags: Vec::new(),
        forbids_flags: Vec::new(),
        requires_state: Vec::new(),
        when_requires: Vec::new(),
        when_forbids: Vec::new(),
        writes: Vec::new(),
        gathered_by: Some("/content/triggers/0/effects/0/steps/0/effects/0".to_string()),
    };
    let all: BTreeSet<[i32; 3]> = (0..12).map(|x| [x, 64, 0]).collect();
    let cells = press_cells_outside(&w, &link, &all);
    assert!(
        cells.contains(&[4, 64, 0]),
        "beside the body, outside the volume: {cells:?}"
    );
    assert!(
        !cells.iter().any(|c| link.contains(*c)),
        "never a cell inside the volume"
    );
    assert!(
        !cells.contains(&[11, 64, 0]),
        "eight cells off is beyond a strike"
    );
    let only_inside: BTreeSet<[i32; 3]> = (0..3).map(|x| [x, 64, 0]).collect();
    assert!(
        press_cells_outside(&w, &link, &only_inside).is_empty(),
        "a cell the walk does not reach is no press cell"
    );
    // Widened over every cell the press reaches from, the volume leaves none.
    link.from = ([0, 64, 0], [7, 65, 0]);
    let wide = press_cells_outside(&w, &link, &all);
    assert!(wide.iter().all(|c| c[0] > 7), "{wide:?}");
}

/// A 3x3 solid plate at y=63 with a water source standing on its `+x` edge
/// column, and a built volume covering exactly the plate. Vanilla runs that
/// source off the plate and down: the shape of every shoreline piece placed
/// against nothing.
/// spec-0083 × spec-0088: **a link's `to` is judged in the region state of
/// its leg** (`DW0932`'s "`to` not standable"), and that state carries every
/// staged lethal volume that may be live there: a landing on floor a volume
/// may hold live is not a cell a body stands on. The same floor with the
/// volume dead is.
#[test]
fn a_link_onto_floor_a_staged_volume_may_hold_live_is_not_standable() {
    let mut blocks: BTreeMap<[i32; 3], String> = BTreeMap::new();
    for x in 0..12 {
        blocks.insert([x, 63, 0], "minecraft:stone".to_string());
    }
    let occ = crate::compiler::assembled::occupancy_of(blocks, &BTreeSet::new());
    let w = World::from_occupancy(occ, Premises::geometry_only());
    let to = [10, 64, 0];
    let link = crate::compiler::link::LinkPlan {
        trigger_id: "trigger/t".to_string(),
        on: "use",
        anchor_id: Some("anchor/a".to_string()),
        npc_id: None,
        assembly_id: None,
        range: None,
        body: vec![[1, 64, 0]],
        path: "/content/triggers/0/effects/0".to_string(),
        from_anchor: "anchor/deck".to_string(),
        from: ([0, 64, 0], [2, 65, 0]),
        from_area: "area/a".to_string(),
        to_anchor: "anchor/landing".to_string(),
        to_area: "area/a".to_string(),
        to,
        tick: 0,
        requires_flags: Vec::new(),
        forbids_flags: Vec::new(),
        requires_state: Vec::new(),
        when_requires: Vec::new(),
        when_forbids: Vec::new(),
        writes: Vec::new(),
        gathered_by: None,
    };
    let dead = RegionState::default();
    assert!(
        to_standable(&w, &dead, &link),
        "the landing floor stands, the volume dead"
    );
    let mut live = RegionState::default();
    let pit = ([10, 64, 0], [10, 64, 0]);
    live.lethal
        .extend(crate::compiler::assembled::region_cells(pit.0, pit.1));
    live.lethal_regions.push(("lethal/pit".to_string(), pit));
    assert!(
        !to_standable(&w, &live, &link),
        "a landing a staged volume may hold live is not standable"
    );
}

/// spec-0086 §5.2: an exercise step is walked to, the carry to its landing
/// is marked like a crossing, and the next leg begins at the landing — the
/// one enumeration every consumer reads.
#[test]
fn an_exercise_step_marks_the_leg_out_of_it_from_the_landing() {
    let steps = vec![
        Step::Loop {
            loop_id: "loop/g".into(),
            pos: [2, 67, 16],
            cross: [2, 67, 22],
            offset: [0, 0, -6],
            times: 2,
            transport: [2, 67, 16],
        },
        Step::Reach {
            objective_id: "obj/end".into(),
            anchor_id: "anchor/end".into(),
            pos: [2, 67, 41],
            radius: 1,
            completion: crate::compiler::reach::reach_completion([2, 67, 41], 1),
        },
    ];
    let transports = vec![Some([2, 67, 16]), None];
    let got: Vec<([i32; 3], bool, usize)> = positions_of(Some([2, 67, 2]), &steps, &transports)
        .iter()
        .map(|p| (p.pos, p.transport_before, p.src_step))
        .collect();
    assert_eq!(
        got,
        vec![
            ([2, 67, 2], false, 0),
            ([2, 67, 16], false, 0),
            ([2, 67, 16], true, 0),
            ([2, 67, 41], false, 1),
        ]
    );
}

fn vp(pos: [i32; 3], transport_before: bool) -> VisitedPos {
    VisitedPos {
        pos,
        transport_before,
        talk_to: false,
        src_step: 0,
        by_link: false,
        by_loop: false,
    }
}

/// The full walked path for `cells`, as the emitter would teleport it.
fn walked(cells: &[[i32; 3]]) -> Vec<[f64; 3]> {
    resample_body(cells, DEFAULT_SPEED, PLAYER_WIDTH).0
}

/// **Why the swept test carries no killing-volume clause of its own, and the
/// property that makes that safe.**
///
/// A smoothed body is off cell centre almost everywhere. Solid geometry is
/// covered regardless, because whole cells tile the plane and the swept test
/// asks `standable_fp` of every column the body overlaps. A lethal volume looks
/// at first like the one question that could not be covered that way — except
/// that `cell_can_meet_volume` never asked about a centred body: a walker's
/// cell does not fix its position, so the predicate already refuses every cell
/// from which a body standing ANYWHERE inside it could reach the volume, and
/// the swept test inherits that reading whole.
///
/// This pins it, because a second rule restating that privately would be the
/// defect rather than the safety: narrow `cell_can_meet_volume` to the centred
/// body and this reds — which is exactly when a diagonal could be smoothed past
/// something that kills.
#[test]
fn the_swept_test_inherits_the_off_centre_reading_of_a_killing_volume() {
    let world = floored_with_lethal(20, 20, 65, ([8, 65, 4], [8, 66, 4]));
    let fp = Footprint::player();
    // The volume's own column and BOTH its neighbours are already refused —
    // half a body width reaches one cell either way.
    for cx in 7..=9 {
        assert!(
            !world.standable_fp([cx, 65, 4], &fp),
            "column {cx} is within a body's reach of the volume and must be kept out"
        );
    }
    assert!(world.standable_fp([6, 65, 4], &fp));
    // So a straight run whose sweep crosses that kept-out ground is refused by
    // the standability clause alone.
    assert!(!world.segment_walkable_fp([7, 65, 1], [7, 65, 8], &fp, PLAYER_WIDTH));
    // ...and one clear of the volume is taken.
    assert!(world.segment_walkable_fp([2, 65, 1], [2, 65, 8], &fp, PLAYER_WIDTH));
}

/// **Regression (owner, island QA): "the NPC visibly passes through blocks".**
///
/// A 1-wide corridor with solid walls on both sides. Every planned waypoint must
/// keep the mover's whole AABB out of the walls. The bare integer cell
/// coordinate — what the emitter used before `cell_center` — puts 70 % of a
/// 0.6-wide body inside the neighbouring column, i.e. inside the wall, for the
/// entire walk; the second half of this test asserts exactly that, so the defect
/// cannot silently come back.
#[test]
fn walked_path_keeps_the_body_out_of_corridor_walls() {
    let y = 65;
    let mut walls = Vec::new();
    for z in 0..8 {
        for dy in 0..2 {
            walls.push([0, y + dy, z]); // west wall
            walls.push([2, y + dy, z]); // east wall
        }
    }
    let world = floored(3, 8, y, &walls);
    let cells: Vec<[i32; 3]> = (0..8).map(|z| [1, y, z]).collect();
    let path = world
        .find_path(cells[0], *cells.last().unwrap())
        .expect("the corridor is walkable");
    assert_eq!(path, cells, "a 1-wide corridor has exactly one route");

    for w in walked(&path) {
        assert!(
            !aabb_clips(&world, w, 0.6),
            "waypoint {w:?} puts the body inside a corridor wall"
        );
    }
    // The pre-fix emission (bare cell coordinates) DID clip — the defect this
    // test guards. Keep as the counter-example, never as the behaviour.
    assert!(
        aabb_clips(&world, [1.0, y as f64, 3.0], 0.6),
        "a body at the bare integer cell straddles the wall columns"
    );
}

/// An L-shaped corridor whose inside corner is solid. A* is strictly cardinal
/// (`neighbors_fp` offers 4 horizontal moves, never a diagonal), so no path can
/// cut the corner; this pins that property *and* proves the interpolated body
/// never enters the corner block on the turn.
#[test]
fn corner_turn_routes_around_the_corner_block_not_through_it() {
    let y = 65;
    // Open cells: the column z=1..=4 at x=1, then x=1..=4 at z=4. Everything
    // else at head height is solid, including the inside corner [2, y, 1].
    let open: BTreeSet<[i32; 3]> = (1..=4)
        .map(|z| [1, y, z])
        .chain((1..=4).map(|x| [x, y, 4]))
        .collect();
    let mut walls = Vec::new();
    for x in 0..6 {
        for z in 0..6 {
            for dy in 0..2 {
                if !open.contains(&[x, y, z]) {
                    walls.push([x, y + dy, z]);
                }
            }
        }
    }
    let world = floored(6, 6, y, &walls);
    let path = world
        .find_path([1, y, 1], [4, y, 4])
        .expect("the L-corridor is walkable");
    assert!(
        path.iter().all(|c| open.contains(c)),
        "the route must stay in the open cells: {path:?}"
    );
    for w in walked(&path) {
        assert!(
            !aabb_clips(&world, w, 0.6),
            "waypoint {w:?} clips the corner block"
        );
    }
}

/// **Regression (owner playtest, island staging round): "a body reads as
/// levitating rather than hopping."**
///
/// A one-block step up is rendered as a **step**: the body is already moving
/// forward while it gains the block, instead of rising in place over the cell
/// it is standing on and then sliding across level.
///
/// This deliberately REPLACES the older assertion that the step is an L whose
/// first leg is a pure vertical translation over the source column: that shape
/// is the finding. What the L bought — the AABB never entering the step block
/// — is re-asserted here and is not given up, and the ceiling sits on the
/// first course above the ledge that no proof covers, so a shape that rose
/// above its landing would clip it.
#[test]
fn a_one_block_step_up_comes_down_onto_the_ledge_and_never_clips_it() {
    let y = 65;
    let h = delvewright_dsl::metrics::PLAYER_HEIGHT;
    let cells_tall = h.ceil() as i32; // the routed footprint's clearance, 2
    let mut solid = BTreeSet::new();
    for x in 0..4 {
        for z in 0..3 {
            solid.insert([x, y - 1, z]); // lower floor
        }
    }
    // A raised ledge at x in {2,3}: its top face is the upper walking surface.
    for x in [2, 3] {
        for z in 0..3 {
            solid.insert([x, y, z]);
        }
    }
    // The ceiling sits on the FIRST course no proof covers: `standable_fp`
    // clears the ledge's own `cells_tall` courses and nothing above them, and
    // `head_clear_to_jump` is checked at the SOURCE column only. A hop that
    // reached higher than the slack between the body and those whole cells
    // would head-bonk here.
    for x in [2, 3] {
        for z in 0..3 {
            solid.insert([x, y + 1 + cells_tall, z]);
        }
    }
    let world = World::from_solid_cells(solid);
    let path = world
        .find_path([1, y, 1], [3, y + 1, 1])
        .expect("a one-block step up is walkable");
    assert_eq!(path, vec![[1, y, 1], [2, y + 1, 1], [3, y + 1, 1]]);

    let pts = walked(&path);
    // Not a translation in place: every emitted segment that changes height
    // also moves horizontally.
    for w in pts.windows(2) {
        let dy = (w[1][1] - w[0][1]).abs();
        let dh = ((w[1][0] - w[0][0]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt();
        assert!(
            dy <= 1e-9 || dh > 1e-9,
            "a walked leg rendered a vertical translation in place: {:?} -> {:?}",
            w[0],
            w[1]
        );
    }
    // The rise happens while the body ADVANCES, and it never goes above its
    // landing — the volume an arc would sweep is not one the route proof
    // clears (see `step_vertices`).
    assert!(
        pts.windows(2).any(|w| {
            let dh = ((w[1][0] - w[0][0]).powi(2) + (w[1][2] - w[0][2]).powi(2)).sqrt();
            w[1][1] > w[0][1] + 1e-9 && dh > 1e-9
        }),
        "the step up never gains height while advancing: {pts:?}"
    );
    let landing = (y + 1) as f64;
    assert!(
        pts.iter().all(|w| w[1] <= landing + 1e-9),
        "a waypoint rose above the ledge it lands on: {pts:?}"
    );
    for w in &pts {
        assert!(
            !aabb_clips_body(&world, *w, PLAYER_WIDTH, h),
            "waypoint {w:?} clips the step block or the ceiling over the ledge"
        );
    }
}

#[test]
fn path_routes_around_a_wall_corner() {
    // A wall spanning z=0..2 at x=2 forces a detour around its open end at z=2.
    let world = floored(5, 4, 65, &[[2, 65, 0], [2, 65, 1], [2, 65, 2]]);
    let path = world.find_path([0, 65, 0], [4, 65, 0]).expect("routable");
    assert_eq!(path.first(), Some(&[0, 65, 0]));
    assert_eq!(path.last(), Some(&[4, 65, 0]));
    // The detour must have turned a corner: it cannot be a straight x-line.
    assert!(
        path.iter().any(|c| c[2] >= 3),
        "path must round the wall's open end, got {path:?}"
    );
    // No waypoint sits inside the wall.
    for c in &path {
        assert!(!world.is_solid(*c), "path clips wall at {c:?}");
    }
}

#[test]
fn disconnected_floors_are_unroutable() {
    // Two floor patches with a void gap (no floor at x=2) → DW0307 condition.
    let mut solid = BTreeSet::new();
    for x in [0, 1, 3, 4] {
        for z in 0..3 {
            solid.insert([x, 64, z]);
            solid.insert([x, 67, z]);
        }
    }
    let world = World::from_solid_cells(solid);
    assert!(world.standable([0, 65, 1]));
    assert!(world.standable([4, 65, 1]));
    assert!(world.find_path([0, 65, 1], [4, 65, 1]).is_none());
}

#[test]
fn critical_path_unroutable_leg_is_dw0311() {
    // Two standable floor patches separated by a void gap (no floor at x=2):
    // a walked leg across them is DW0311; the same leg guarded by a transport
    // hop (transport_before = true) is skipped.
    let mut solid = BTreeSet::new();
    for x in [0, 1, 3, 4] {
        for z in 0..3 {
            solid.insert([x, 64, z]);
            solid.insert([x, 67, z]);
        }
    }
    let world = World::from_solid_cells(solid);
    let a = [0, 65, 1];
    let b = [4, 65, 1];
    assert!(world.standable(a) && world.standable(b));
    // Walked leg → unroutable → DW0311.
    let err = route_visited(
        &world,
        &[vp(a, false), vp(b, false)],
        &RegionEvents::default(),
        &linear,
    )
    .unwrap_err();
    assert_eq!(err.code, DW_CRITICAL_UNROUTABLE);
    // Same leg ridden by an inter-area transport → skipped, ok.
    assert!(
        route_visited(
            &world,
            &[vp(a, false), vp(b, true)],
            &RegionEvents::default(),
            &linear
        )
        .is_ok()
    );
}

#[test]
fn talk_to_endpoint_excludes_the_npc_cell_and_flooded_cells() {
    // A flat floor; the NPC anchor cell is standable (a mannequin stands on the
    // floor), and one adjacent cell is flooded. The talk-to goal snap must NOT
    // return the NPC's own cell, and must skip the flooded neighbour — it lands
    // on a dry standable cell beside the NPC, within interaction range.
    let mut solid = BTreeSet::new();
    for x in 0..5 {
        for z in 0..5 {
            solid.insert([x, 64, z]); // floor at y=64, standable at y=65
        }
    }
    let npc = [2, 65, 2];
    let flooded: BTreeSet<[i32; 3]> = [[1, 65, 2]].into_iter().collect(); // west neighbour is water
    let world = World::from_solid_and_flooded(solid, flooded);
    assert!(
        world.standable(npc),
        "the NPC cell itself is standable in the model"
    );
    let goal = world
        .snap_endpoint(npc, true)
        .expect("a dry standable cell beside the NPC exists");
    assert_ne!(goal, npc, "must not stand on the NPC's own (occupied) cell");
    assert!(
        !world.flooded.contains(&goal),
        "must not stand in water: {goal:?}"
    );
    assert!(world.standable(goal));
    // …and it is within interaction range (adjacent) of the NPC.
    let d2 = (0..3).map(|i| (goal[i] - npc[i]).pow(2)).sum::<i32>();
    assert!(
        d2 <= SNAP_RADIUS * SNAP_RADIUS,
        "goal {goal:?} within range of NPC"
    );
}

#[test]
fn verify_exported_routes_rejects_a_flooded_waypoint_dw0314() {
    // Synthetic negative for the DW0314 self-check: a hand-built leg
    // whose polyline crosses a flooded cell must fail the standability guard.
    let mut solid = BTreeSet::new();
    for x in 0..4 {
        solid.insert([x, 64, 0]); // floor
    }
    let flooded: BTreeSet<[i32; 3]> = [[2, 65, 0]].into_iter().collect(); // a water tongue on the route
    let world = World::from_solid_and_flooded(solid, flooded);
    let routes = vec![LegRoute {
        from: [0, 65, 0],
        to: [3, 65, 0],
        to_step: 1,
        cells: vec![[0, 65, 0], [1, 65, 0], [2, 65, 0], [3, 65, 0]],
        use_gates: Vec::new(),
        climbs: Vec::new(),
        // No runtime write on this leg: the bare world is the world it was
        // proven over, so the flooded cell has nothing to explain it away.
        region_state: RegionState::default(),
    }];
    let err = verify_exported_routes(&world, &routes).unwrap_err();
    assert_eq!(err.code, DW_WAYPOINT_NOT_STANDABLE);
    assert!(
        err.message.contains("[2, 65, 0]"),
        "names the offending cell: {}",
        err.message
    );
    // A route entirely on dry standable floor passes.
    let dry = vec![LegRoute {
        from: [0, 65, 0],
        to: [1, 65, 0],
        to_step: 1,
        cells: vec![[0, 65, 0], [1, 65, 0]],
        use_gates: Vec::new(),
        climbs: Vec::new(),
        region_state: RegionState::default(),
    }];
    assert!(verify_exported_routes(&world, &dry).is_ok());
}

#[test]
fn critical_path_routable_leg_passes() {
    // A flat connected floor: consecutive visited cells are walkable → ok.
    let world = floored(6, 3, 65, &[]);
    assert!(
        route_visited(
            &world,
            &[vp([0, 65, 1], false), vp([5, 65, 1], false)],
            &RegionEvents::default(),
            &linear
        )
        .is_ok()
    );
}

/// The gateless ram-pen shape: a closed fence ring at stand level around an
/// interior anchor. `gate`, when set, replaces one ring cell with a closed
/// fence gate (a use-gate cell).
fn fence_ring_world(gate: Option<[i32; 3]>) -> World {
    let y = 65;
    let mut ring: Vec<[i32; 3]> = Vec::new();
    for i in 1..=5 {
        ring.push([i, y, 1]);
        ring.push([i, y, 5]);
        ring.push([1, y, i]);
        ring.push([5, y, i]);
    }
    let gates: Vec<[i32; 3]> = gate.into_iter().collect();
    ring.retain(|c| !gates.contains(c));
    classified(7, 7, y, &ring, &gates)
}

#[test]
fn gateless_fence_ring_is_dw0311() {
    // The soundness hole the full-solid model had: a pen fenced on every side
    // with NO gate "passed" the completability proof by standing the player on
    // the fence-top. It must now be a DW0311 build failure.
    let world = fence_ring_world(None);
    let inside = [3, 65, 3];
    let outside = [0, 65, 0]; // corner, outside the ring
    assert!(world.standable(inside) && world.standable(outside));
    let err = route_visited(
        &world,
        &[vp(outside, false), vp(inside, false)],
        &RegionEvents::default(),
        &linear,
    )
    .expect_err("a humanly impassable gateless fence ring must fail the proof");
    assert_eq!(err.code, DW_CRITICAL_UNROUTABLE); // DW0311
    assert!(
        err.message.contains("fence"),
        "the message should name the barrier class: {}",
        err.message
    );
}

#[test]
fn fence_ring_with_closed_gate_routes_through_it_as_a_use_gate_edge() {
    // The island ram pen: the ring's only opening is a closed oak_fence_gate.
    // The player passes it with an adventure-legal right-click, so the proof
    // routes THROUGH the gate cell — a first-class use-gate edge, not a
    // fence-top hop and not a harness workaround.
    let gate = [3, 65, 1];
    let world = fence_ring_world(Some(gate));
    let inside = [3, 65, 3];
    let outside = [3, 65, 0];
    let path = world
        .find_path(outside, inside)
        .expect("the pen is enterable through its gate");
    assert!(
        path.contains(&gate),
        "the proven route must pass through the gate cell: {path:?}"
    );
    assert!(world.is_use_gate(gate), "the gate cell is tagged use-gate");
    // Every route cell is standable in the final model (the DW0314 guard).
    for &c in &path {
        assert!(world.is_standable(c), "route cell {c:?} standable");
    }
    assert!(
        route_visited(
            &world,
            &[vp(outside, false), vp(inside, false)],
            &RegionEvents::default(),
            &linear
        )
        .is_ok()
    );
    // The gate is still never a floor: its top is not standable.
    assert!(!world.standable([3, 66, 1]));
}

#[test]
fn autonomous_walkers_treat_a_closed_gate_as_a_fence() {
    // A wave mob acting on its own cannot right-click: on the no-gate-use view
    // (wave seating) the pen is sealed again.
    let gate = [3, 65, 1];
    let world = fence_ring_world(Some(gate));
    let entity_world = world.without_gate_use();
    assert!(world.has_use_gates() && !entity_world.has_use_gates());
    assert!(
        entity_world.find_path([3, 65, 0], [3, 65, 3]).is_none(),
        "a non-player walker must not route through a closed gate"
    );
    // Wave seating never picks the gate threshold or anything past it.
    let cells = entity_world.confined_standable_cells([3, 65, 3], ([2, 64, 2], [4, 66, 4]));
    assert!(!cells.is_empty());
    assert!(!cells.contains(&gate), "no mob seated in the gate cell");
}

#[test]
fn critical_path_route_returns_the_proven_cell_polyline() {
    // A flat connected floor: the walked leg's exported route is the A* cell
    // path, inclusive of both snapped endpoints.
    let world = floored(8, 3, 65, &[]);
    let a = [0, 65, 1];
    let b = [6, 65, 1];
    let cells = world.find_path(a, b).expect("routable");
    assert_eq!(cells.first(), Some(&a));
    assert_eq!(cells.last(), Some(&b));
    // Every cell on an exported route is standable (a real floor cell).
    for c in &cells {
        assert!(world.standable(*c), "route cell {c:?} not standable");
    }
}

#[test]
fn exported_waypoints_never_route_through_a_sealed_gate() {
    // The z=0 lane is the short way; a `close-gate` seals its chokepoint
    // [2,65,0] before the leg is walked. The completability proof already
    // routed the detour — the EXPORT must agree, or the harness bot is handed
    // a route through a boulder that has already dropped.
    let mut world = two_lane_room(65);
    // Join the two lanes at both ends so a detour exists.
    world.solid.remove(&[0, 65, 1]);
    world.solid.remove(&[0, 66, 1]);
    world.solid.remove(&[4, 65, 1]);
    world.solid.remove(&[4, 66, 1]);
    let a = at_step([0, 65, 0], 1);
    let b = at_step([4, 65, 0], 2);
    let close = RegionEvent::forced(([2, 65, 0], [2, 66, 0]), RegionWrite::Fill, 0);
    let open_legs = route_walked_legs(&world, &[a, b], &RegionEvents::default(), &linear);
    assert!(
        open_legs[0].0.cells.contains(&[2, 65, 0]),
        "with the gate open the export takes the short lane"
    );
    let sealed_legs = route_walked_legs(
        &world,
        &[a, b],
        &RegionEvents::from(vec![close.clone()]),
        &linear,
    );
    assert_eq!(sealed_legs.len(), 1, "the leg is still routable via z=2");
    assert!(
        !sealed_legs[0].0.cells.contains(&[2, 65, 0]),
        "an exported waypoint must never cross a sealed gate cell: {:?}",
        sealed_legs[0].0.cells
    );
    assert!(
        sealed_legs[0].0.cells.contains(&[2, 65, 2]),
        "the export takes the detour lane the proof routed"
    );
}

// --- terrain-shaped step cost (round-8 owner playtest) -------------------

/// An open plateau: solid floor at `y=63` over `[0,w) × [0,d)`, so the walk
/// plane is `y=64`. Each cell in `bumps` gets a block laid ON the floor, which
/// raises the standing cell there by one — the "bumpy 1-step terrain" the herd
/// and the giant pogo'd over on the island.
fn plateau(w: i32, d: i32, bumps: &[[i32; 2]]) -> World {
    let mut solid = BTreeSet::new();
    for x in 0..w {
        for z in 0..d {
            solid.insert([x, 63, z]);
        }
    }
    for &[x, z] in bumps {
        solid.insert([x, 64, z]);
    }
    World::from_solid_cells(solid)
}

/// The island defect in miniature: a straight lane with one 1-block bump, and
/// a flat lane one column over. The flat road is two steps longer and must
/// still win — this is precisely what a distance-only cost could not do.
#[test]
fn a_walk_takes_a_slightly_longer_flat_lane_over_a_bump() {
    // Lane x=0 carries a bump at z=5; lane x=1 is clear.
    let world = plateau(4, 11, &[[0, 5]]);
    let path = world
        .find_path([0, 64, 0], [0, 64, 10])
        .expect("both lanes connect the endpoints");
    assert!(
        !path.contains(&[0, 65, 5]),
        "the planner must route around the bump, not over it: {path:?}"
    );
    assert!(
        path.iter().any(|c| c[0] == 1),
        "the detour uses the flat lane one column over: {path:?}"
    );
    // Every cell of the chosen route is level — the whole point.
    assert!(
        path.iter().all(|c| c[1] == 64),
        "the flat route stays on one plane: {path:?}"
    );
}

/// The other side of the constant: flatness is preferred, not bought at any
/// price. With the three nearest lanes all bumped, the flat lane is three
/// columns away (six extra steps) — more than one bump is worth — so the walk
/// correctly steps over the bump instead of touring the map to avoid it.
#[test]
fn a_walk_does_not_take_an_absurd_detour_to_avoid_one_bump() {
    let world = plateau(5, 11, &[[0, 5], [1, 5], [2, 5]]);
    let path = world
        .find_path([0, 64, 0], [0, 64, 10])
        .expect("the bumped lanes are still walkable");
    assert!(
        path.iter().any(|c| c[1] == 65),
        "a 6-step detour costs more than one 1-block step up: {path:?}"
    );
    assert!(
        !path.iter().any(|c| c[0] == 3),
        "the far flat lane is not worth the detour: {path:?}"
    );
}

/// Cost shaping changes which of several *valid* routes is chosen — never
/// which routes exist. A bump is still walkable when it is the only way, and
/// a genuinely disconnected goal is still unreachable (DW0307/DW0311 semantics
/// unchanged).
#[test]
fn cost_shaping_does_not_change_reachability() {
    // A single lane, bumped: the only route climbs, and it is still found.
    let world = plateau(1, 11, &[[0, 5]]);
    let path = world
        .find_path([0, 64, 0], [0, 64, 10])
        .expect("a bump is a cost, never a wall");
    assert!(path.contains(&[0, 65, 5]));
    // A wall two blocks tall is still impassable.
    let mut solid = BTreeSet::new();
    for x in 0..1 {
        for z in 0..11 {
            solid.insert([x, 63, z]);
        }
    }
    solid.insert([0, 64, 5]);
    solid.insert([0, 65, 5]);
    solid.insert([0, 66, 5]);
    let walled = World::from_solid_cells(solid);
    assert!(walled.find_path([0, 64, 0], [0, 64, 10]).is_none());
}

/// Determinism (ADR-0006): the same world and endpoints yield the identical
/// path every time — the frontier is ordered by `(f, g, cell)` and the costs
/// are integers, so no float comparison or map ordering can wobble the result.
#[test]
fn shaped_paths_are_deterministic() {
    let world = plateau(5, 11, &[[0, 5], [2, 3], [3, 7]]);
    let first = world.find_path([0, 64, 0], [4, 64, 10]).unwrap();
    for _ in 0..8 {
        assert_eq!(world.find_path([0, 64, 0], [4, 64, 10]).unwrap(), first);
    }
}

/// A floor at y = 64 over x 0..=6, z 0..=2, and a solid mass at x 3..=6 up to
/// y = 68, so its top is stood on at y = 69. `ladder` hangs at x = 2, z = 1,
/// y 65..=68, in the block state given.
fn ladder_cliff(ladder: Option<&str>) -> World {
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..=6 {
        for z in 0..=2 {
            cells.push(([x, 64, z], "minecraft:stone"));
            if x >= 3 {
                for y in 65..=68 {
                    cells.push(([x, y, z], "minecraft:stone"));
                }
            }
        }
    }
    if let Some(l) = ladder {
        for y in 65..=68 {
            cells.push(([2, y, 1], l));
        }
    }
    blocks_world(&cells)
}

/// **A forced leg up a ladder is proven, and one up a ladder the world does not
/// keep is `DW0991`, naming it.** The same four-course face three ways: a ladder
/// that hangs (the leg routes), a ladder turned to face into the mass with
/// nothing behind it (`DW0991`, the block and the hold it lacks), and no ladder
/// at all (`DW0311`, the generic answer — there is nothing to blame).
#[test]
fn a_leg_up_a_ladder_is_proven_and_an_unheld_ladder_is_dw0991() {
    let (a, b) = ([0, 65, 1], [5, 69, 1]);
    let leg = [vp(a, false), vp(b, false)];
    assert!(
        route_visited(
            &ladder_cliff(Some("minecraft:ladder[facing=west]")),
            &leg,
            &RegionEvents::default(),
            &linear
        )
        .is_ok()
    );
    let err = route_visited(
        &ladder_cliff(Some("minecraft:ladder[facing=east]")),
        &leg,
        &RegionEvents::default(),
        &linear,
    )
    .unwrap_err();
    assert_eq!(err.code, DW_CLIMB_UNHELD, "{}", err.message);
    assert!(err.message.contains("[2, 66, 1]"), "{}", err.message);
    assert!(err.message.contains("sturdy east face"), "{}", err.message);
    let bare =
        route_visited(&ladder_cliff(None), &leg, &RegionEvents::default(), &linear).unwrap_err();
    assert_eq!(bare.code, DW_CRITICAL_UNROUTABLE);
}

/// **The exported route carries its climb, and its ends are waypoints.**
#[test]
fn the_exported_leg_names_its_climb() {
    let w = ladder_cliff(Some("minecraft:ladder[facing=west]"));
    let routes = route_walked_legs(
        &w,
        &[vp([0, 65, 1], false), vp([5, 69, 1], false)],
        &RegionEvents::default(),
        &linear,
    );
    let leg = &routes[0].0;
    assert_eq!(leg.climbs.len(), 1);
    assert!(
        verify_exported_routes(&w, std::slice::from_ref(leg)).is_ok(),
        "a held cell is a cell a body is in"
    );
}
