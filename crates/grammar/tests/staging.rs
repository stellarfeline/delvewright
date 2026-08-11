//! The staging vocabulary's gates: W1 — the knockback niche (`cliff_path`) and
//! the watch bay (`watch_bay`) — and W2 — the rafter perch (`rafter_hall`), the
//! corner-ambush alcove (`ambush_door`) and the container tell (`store_room`).
//!
//! These rules exist for their **gates**, not for their prettiness. A niche is
//! only a knockback test if the recess is shallow enough to swing into and the
//! ledge is the only way past; a bay is only observability hardware if you can
//! actually see the hazard from it; a rafter is only fair if the doorway can see
//! it, and an alcove is only an ambush if the doorway cannot. Every one of those
//! claims is geometry, so every one is asserted here against the expanded model
//! rather than described in prose — and every one is shown going red, because a
//! gate nobody has watched fail proves nothing.
//!
//! The sightline walk below is deliberately the same shape as the compiler's
//! `DW0388` proof (`crates/compiler/src/nav.rs`: eye at 1.62 over the watch
//! cell, target at 1.0 over the observed cell, Amanatides–Woo cell traversal,
//! both endpoint cells exempt). It is a *generator-level* guarantee — the
//! campaign-level proof still runs on the assembled world with real hazard
//! declarations. What this file proves is that the bay a rule places **can**
//! satisfy that proof, so the compiler is never handed geometry it must reject.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_grammar::block::BlockState;
use delvewright_grammar::ir::Program;
use delvewright_grammar::library::bait_stand::{self, bait_stand};
use delvewright_grammar::library::causeway::MIN_GATE_RISE;
use delvewright_grammar::library::disarm_stand::{self, disarm_stand};
use delvewright_grammar::library::elite_ground::MIN_RADIUS;
use delvewright_grammar::library::hearth_ward::{self, hearth_ward};
use delvewright_grammar::library::rafter_hall::FLOOR_CELLS_PER_PERCH;
use delvewright_grammar::library::stair_flight::{self, stair_flight};
use delvewright_grammar::library::tee_passage::{self, tee_passage};
use delvewright_grammar::library::{
    ambush_door, causeway, cliff_path, drop_shaft, dumbwaiter, elite_ground, far_side_bar,
    rafter_hall, store_room, watch_bay,
};
use delvewright_grammar::{Anchor, Box3, ExpandOptions, Expansion, VoxelModel, expand};
// W3: the palette/prop family (W + S + M + X).
use delvewright_grammar::library::boulder_stair::{self, boulder_stair};
use delvewright_grammar::library::broken_grate::{self, broken_grate};
use delvewright_grammar::library::threshold_motif::{self, threshold_motif};

/// The cliff path fixture: three block wide (ledge, recess lane, backing),
/// six tall, thirty long.
const CLIFF_REGION: Box3 = Box3::at_origin([3, 6, 30]);
/// The seed the three-niche fixture is pinned to.
const CLIFF_SEED: u64 = 4;

/// The gate passage fixture, comfortably over every declared minimum.
const PASSAGE_REGION: Box3 = Box3::at_origin([7, 7, 24]);
/// The seed the passage fixture is pinned to. `watch_bay` has no probabilistic
/// rule, so the seed only has to be *stated*, not chosen.
const PASSAGE_SEED: u64 = 1;

/// The rafter hall fixture: eleven cells of nave between the side walls, exactly
/// tall enough for the truss, long enough to carry seven of them.
const HALL_REGION: Box3 = Box3::at_origin([13, 6, 25]);
/// The same box one course shorter — under `TRUSS_MIN_HEIGHT`, so the hall is
/// legal and has no rafters in it.
const SHORT_HALL_REGION: Box3 = Box3::at_origin([13, 5, 25]);
/// `rafter_hall` has no probabilistic rule; the seed is stated, not chosen.
const HALL_SEED: u64 = 1;

/// The threshold fixture. Longer than it is wide so the frame's `Largest` puts
/// travel on world `Z`, which is what the module diagram draws.
const DOOR_REGION: Box3 = Box3::at_origin([11, 5, 13]);
/// `ambush_door` has no probabilistic rule either.
const DOOR_SEED: u64 = 1;

/// The storeroom fixture: a fourteen-barrel row.
const STORE_REGION: Box3 = Box3::at_origin([7, 5, 14]);

/// The worn-tread lane fixture: nine wide, long enough for four pockets at
/// the default eight-cell period. `boulder_stair` has no probabilistic rule
/// — pocket placement follows `pocket_period`, not the seed — so the seed
/// only has to be stated.
const STAIR_REGION: Box3 = Box3::at_origin([9, 6, 27]);
const STAIR_SEED: u64 = 1;
/// The same width, five deep: short of even one `pocket_period`, so the
/// pocket band is legally solid and bare.
const SHORT_STAIR_REGION: Box3 = Box3::at_origin([5, 6, 7]);
/// A narrower cut of the lane, chosen so the smooth run's *floor-level* share
/// clears the 10% accent ceiling — the fixture the palette-mirror gate needs
/// to have real teeth (`docs/reference/grammar.md` §7: the craft diagnostics
/// are a later phase, so this is a test-local mirror, not `DW0388`-style
/// reuse of compiler code).
const STAIR_PALETTE_REGION: Box3 = Box3::at_origin([7, 6, 9]);

/// The threshold fixture, narrow and wide, both long enough to hold the
/// doorband: `threshold_motif` has no probabilistic rule either.
const THRESHOLD_REGION: Box3 = Box3::at_origin([9, 6, 13]);
const WIDE_THRESHOLD_REGION: Box3 = Box3::at_origin([15, 6, 19]);
const THRESHOLD_SEED: u64 = 1;

/// The broken-grate fixture: a fourteen-cell row, the same shape as
/// `store_room`'s.
const GRATE_REGION: Box3 = Box3::at_origin([3, 5, 14]);
/// Short enough that one broken cell's floor-level share clears the 10%
/// accent ceiling — the palette-mirror gate's fixture, the same reasoning as
/// `STAIR_PALETTE_REGION`.
const GRATE_PALETTE_REGION: Box3 = Box3::at_origin([3, 5, 8]);

/// The drop shaft fixture: comfortably over its documented minimum.
const SHAFT_REGION: Box3 = Box3::at_origin([4, 8, 6]);
const SHAFT_SEED: u64 = 1;
/// A short-drop fixture built specifically for the `rescue_ladder` teeth test
/// — see the module note on `drop_shaft::drop_shaft` for why one notch only
/// bridges a two-block drop. `Z` must stay strictly longer than `X`: an equal
/// pair is a tie for "the longer horizontal axis" and the frame's `Largest`
/// reorientation is free to break it either way.
const SHAFT_TEETH_REGION: Box3 = Box3::at_origin([4, 5, 6]);

/// The dumbwaiter fixture: comfortably over its documented minimum.
const DUCT_REGION: Box3 = Box3::at_origin([6, 8, 8]);
const DUCT_SEED: u64 = 1;
const DUCT_TEETH_REGION: Box3 = Box3::at_origin([6, 5, 8]);

/// The stair-flight fixture: five across (a wall, a three-wide lane, a wall),
/// fourteen tall, twenty-two long. Deliberately long enough that the **run**,
/// not the headroom, is what stops the climb — `broken_step`'s guard reads the
/// remaining run, so a fixture whose `Y` ran out first would put the break
/// nowhere. Eight treads, seven blocks of rise; the gates pin both.
const FLIGHT_REGION: Box3 = Box3::at_origin([5, 14, 22]);
/// `stair_flight` draws nothing from the seed; it is stated, not chosen.
const FLIGHT_SEED: u64 = 1;

/// The far-side bar fixture. `Z` strictly longer than `X`, as `SHAFT_TEETH_REGION` notes.
const BAR_REGION: Box3 = Box3::at_origin([5, 5, 7]);
const BAR_SEED: u64 = 1;

/// The tee-passage fixture: a lane long enough that the doorway has real wall on
/// both sides of it, and `Z` strictly longer than `X` as `SHAFT_TEETH_REGION`
/// notes. `tee_passage` has no probabilistic rule; the seed is stated.
const TEE_REGION: Box3 = Box3::at_origin([5, 5, 12]);
const TEE_SEED: u64 = 1;

/// The causeway fixture.
const CAUSEWAY_REGION: Box3 = Box3::at_origin([7, 10, 9]);
const CAUSEWAY_SEED: u64 = 1;

/// The elite-ground fixture, at `radius`'s own enforced floor of 4 (a 9×9
/// circle). Padded past the documented minimum on `Z`: the flank margins make
/// `X` the wider of the two at the minimum itself, and `Z` has to stay
/// strictly the longer axis or the frame's `Largest` reorientation is free to
/// swap which world axis this file's `c[0]`/`c[2]` reads mean.
const ARENA_REGION: Box3 = Box3::at_origin([19, 5, 25]);
const ARENA_SEED: u64 = 1;

// ---------------------------------------------------------------------------
// Reading the expanded model the way a player meets it
// ---------------------------------------------------------------------------

/// Cells a body and a sightline pass through.
///
/// The grammar's terminals in these programs are stone, timber, barrels and a
/// floor skull. A skull is neither a barrier nor an occluder, and saying so
/// matters: the teaching niche has one on the exact cell its anchor names, so a
/// naive "not air means solid" predicate would report that niche as unreachable
/// and invisible. Everything else is a full block, barrels included — a body
/// stands on top of a barrel, not inside it. Outside the region counts as
/// blocking: a ray that leaves the prefab has left the thing being proved.
fn passable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    match model.get(pos) {
        None => false,
        Some(block) => block.is_air() || block.name.ends_with("_skull"),
    }
}

/// A full block: what a floor is made of, and what stops an eye.
fn solid(model: &VoxelModel, pos: [i32; 3]) -> bool {
    model.get(pos).is_some() && !passable(model, pos)
}

/// A cell a player can stand in: two blocks of clearance over a full floor.
fn standable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    let [x, y, z] = pos;
    passable(model, pos) && passable(model, [x, y + 1, z]) && solid(model, [x, y - 1, z])
}

/// Every standable cell of the model.
fn standable_cells(model: &VoxelModel) -> BTreeSet<[i32; 3]> {
    model
        .region()
        .positions()
        .filter(|&p| standable(model, p))
        .collect()
}

/// Can a walker get from any cell of `from` to any cell of `to`, moving one
/// cell horizontally at a time and stepping at most one block up or down?
fn connected(
    cells: &BTreeSet<[i32; 3]>,
    from: &BTreeSet<[i32; 3]>,
    to: &BTreeSet<[i32; 3]>,
) -> bool {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> =
        from.iter().copied().filter(|c| cells.contains(c)).collect();
    seen.extend(queue.iter().copied());
    while let Some([x, y, z]) = queue.pop_front() {
        if to.contains(&[x, y, z]) {
            return true;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in [0, 1, -1] {
                let next = [x + dx, y + dy, z + dz];
                if cells.contains(&next) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
        }
    }
    false
}

/// `connected`'s ±1-step walk, plus a one-way **fall**: stepping off a
/// standable cell into an adjacent column with nothing underfoot, and landing
/// on the first solid floor below however far that is. This is the L family's
/// (`drop_shaft`, `dumbwaiter`) forward-reachability model, and it is
/// deliberately *more* permissive than `crate::nav`'s NPC pathing
/// (`reachable_walkable`), which never risks more than a one-block drop — an
/// escorted NPC never has to, but a player can walk off a ledge. It is used
/// only going forward: proving the L family's "no way back" claim under this
/// extra freedom, and not just under the stricter NPC model, is the stronger
/// claim (see each rule's module note).
///
/// **What it does NOT model: a horizontal jump.** Both this and [`connected`]
/// move one cell at a time with a ±1 height step, so a player's running jump
/// across a gap is invisible to them. Every "unreachable" this file proves
/// therefore means *unreachable by walking and falling*, not unreachable. The
/// consequence differs per rule and is stated here rather than left to be
/// re-derived:
///
/// * `drop_shaft` / `dumbwaiter` — unaffected in the direction that matters.
///   Their claim is that you cannot get back **up**, and a jump does not lift
///   you; the fall model is already the permissive side.
/// * `far_side_bar` — **this is where it bites.** The rule's whole point is
///   that the near side cannot reach `anchor/unlock` while the bar is down. A
///   gap a player can jump would defeat that and this model would not see it,
///   so the fixture must not leave one: keep the separation wider than a jump,
///   or solid.
/// * `elite_ground` — safe. A jump can only *add* flank routes, so the model
///   under-counts, and the gate cares about there being enough, not few.
///
/// House precedent for documenting a model's limit where the model lives:
/// `harness/src/teardown.ts`, which states its own under-classification.
fn reachable_with_fall(
    model: &VoxelModel,
    cells: &BTreeSet<[i32; 3]>,
    from: &BTreeSet<[i32; 3]>,
    to: &BTreeSet<[i32; 3]>,
) -> bool {
    let mut seen: BTreeSet<[i32; 3]> = BTreeSet::new();
    let mut queue: VecDeque<[i32; 3]> =
        from.iter().copied().filter(|c| cells.contains(c)).collect();
    seen.extend(queue.iter().copied());
    while let Some([x, y, z]) = queue.pop_front() {
        if to.contains(&[x, y, z]) {
            return true;
        }
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in [0, 1, -1] {
                let next = [x + dx, y + dy, z + dz];
                if cells.contains(&next) && seen.insert(next) {
                    queue.push_back(next);
                }
            }
            // A fall: walk the adjacent column down from foot height until it
            // hits solid ground, and land there if that landing is standable.
            // However far the drop, this only ever adds an edge downward.
            let mut fy = y;
            loop {
                fy -= 1;
                if y - fy > 64 {
                    break; // not a real shaft, a runaway search
                }
                let below = [x + dx, fy, z + dz];
                match model.get(below) {
                    None => break, // fell out of the structure entirely
                    Some(_) if solid(model, below) => {
                        let landing = [x + dx, fy + 1, z + dz];
                        if standable(model, landing) && seen.insert(landing) {
                            queue.push_back(landing);
                        }
                        break;
                    }
                    _ => continue, // still falling
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Sightline — the same walk `DW0388` uses
// ---------------------------------------------------------------------------

/// Player eye height over the cell they stand in (vanilla 1.62), as `DW0388`.
const EYE_HEIGHT: f64 = 1.62;

/// Whether an observer standing on `watch` can see the space a body standing on
/// `target` occupies. Both endpoint cells are exempt: they are the observer's
/// own head and the volume being looked at.
fn sees(model: &VoxelModel, watch: [i32; 3], target: [i32; 3]) -> Result<(), [i32; 3]> {
    let eye = [
        watch[0] as f64 + 0.5,
        watch[1] as f64 + EYE_HEIGHT,
        watch[2] as f64 + 0.5,
    ];
    let mass = [
        target[0] as f64 + 0.5,
        target[1] as f64 + 1.0,
        target[2] as f64 + 0.5,
    ];
    let eye_cell = [watch[0], watch[1] + 1, watch[2]];
    match walk_cells(eye, mass, |c| {
        c != eye_cell && c != target && !passable(model, c)
    }) {
        Some(cell) => Err(cell),
        None => Ok(()),
    }
}

/// Walk every unit cell the segment `a → b` passes through, in order, returning
/// the first for which `hit` holds. Amanatides–Woo voxel traversal, ported in
/// shape from `crates/compiler/src/nav.rs` so the generator's idea of a
/// sightline and the compiler's cannot drift apart.
fn walk_cells(a: [f64; 3], b: [f64; 3], hit: impl Fn([i32; 3]) -> bool) -> Option<[i32; 3]> {
    let mut cell = [
        a[0].floor() as i32,
        a[1].floor() as i32,
        a[2].floor() as i32,
    ];
    let end = [
        b[0].floor() as i32,
        b[1].floor() as i32,
        b[2].floor() as i32,
    ];
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let mut step = [0i32; 3];
    let mut t_max = [f64::INFINITY; 3];
    let mut t_delta = [f64::INFINITY; 3];
    for i in 0..3 {
        if d[i] > 0.0 {
            step[i] = 1;
            t_max[i] = ((cell[i] + 1) as f64 - a[i]) / d[i];
            t_delta[i] = 1.0 / d[i];
        } else if d[i] < 0.0 {
            step[i] = -1;
            t_max[i] = (cell[i] as f64 - a[i]) / d[i];
            t_delta[i] = -1.0 / d[i];
        }
    }
    if hit(cell) {
        return Some(cell);
    }
    let budget: i64 = (0..3)
        .map(|i| (end[i] - cell[i]).unsigned_abs() as i64)
        .sum();
    for _ in 0..budget {
        let axis = if t_max[0] <= t_max[1] && t_max[0] <= t_max[2] {
            0
        } else if t_max[1] <= t_max[2] {
            1
        } else {
            2
        };
        cell[axis] += step[axis];
        t_max[axis] += t_delta[axis];
        if hit(cell) {
            return Some(cell);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Shared plumbing
// ---------------------------------------------------------------------------

fn expand_at(program: &Program, region: Box3, seed: u64) -> Expansion {
    expand(program, region, &ExpandOptions::seeded(seed))
        .unwrap_or_else(|e| panic!("{}: {e}", program.name))
}

/// Anchors whose exported name starts with `anchor/<stem>-`, in index order.
fn indexed(anchors: &BTreeMap<String, Anchor>, stem: &str) -> Vec<[i32; 3]> {
    let prefix = format!("anchor/{stem}-");
    let mut found: Vec<(u32, [i32; 3])> = anchors
        .iter()
        .filter_map(|(name, a)| {
            let n: u32 = name.strip_prefix(&prefix)?.parse().ok()?;
            Some((n, a.pos))
        })
        .collect();
    found.sort_unstable();
    found.into_iter().map(|(_, pos)| pos).collect()
}

// ---------------------------------------------------------------------------
// K — the knockback niche
// ---------------------------------------------------------------------------

/// The fixture: a three-niche path, byte-identical on a second expansion,
/// anchors and all.
#[test]
fn a_three_niche_cliff_path_expands_deterministically() {
    let program = cliff_path();
    let a = expand_at(&program, CLIFF_REGION, CLIFF_SEED);
    let b = expand_at(&program, CLIFF_REGION, CLIFF_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);

    let niches = indexed(&a.anchors, "niche");
    let watches = indexed(&a.anchors, "niche-watch");
    assert_eq!(niches.len(), 3, "the pinned fixture is a three-niche path");
    assert_eq!(
        watches.len(),
        niches.len(),
        "every recess owes a watch cell: {:#?}",
        a.anchors
    );
}

/// The gate the owner asked for by name: the recess is **one** deep, so an
/// occupant's hitbox sits inside it and a swing from the ledge reaches it. One
/// deeper and the niche becomes a room the player has to enter; one shallower
/// and there is no niche at all.
#[test]
fn every_recess_is_exactly_one_deep_and_two_high() {
    let out = expand_at(&cliff_path(), CLIFF_REGION, CLIFF_SEED);
    let model = &out.model;
    let niches = indexed(&out.anchors, "niche");
    assert!(!niches.is_empty());
    for pos in niches {
        let [x, y, z] = pos;
        assert_eq!(x, 1, "a recess sits one cell in from the ledge lane");
        assert!(passable(model, [x, y, z]), "the recess floor cell {pos:?}");
        assert!(passable(model, [x, y + 1, z]), "the recess is two high");
        assert!(
            solid(model, [x + 1, y, z]),
            "the recess is exactly one deep — {:?} must be backing wall",
            [x + 1, y, z]
        );
        assert!(
            solid(model, [x, y - 1, z]),
            "the recess has a floor to stand on"
        );
        assert!(
            solid(model, [x, y + 2, z]),
            "the recess has a lintel — it is a niche, not a doorway"
        );
        assert!(
            passable(model, [x - 1, y, z]),
            "the recess opens onto the ledge lane"
        );
    }
}

/// The second gate: the ledge along the drop face is the **only** route. A
/// niche off to the side of a wide path is decoration; a niche off a lane you
/// cannot avoid is a test. Cut the lane and the path must be severed.
#[test]
fn the_ledge_lane_is_the_only_route_past_the_niches() {
    let out = expand_at(&cliff_path(), CLIFF_REGION, CLIFF_SEED);
    let model = &out.model;
    let cells = standable_cells(model);
    let far = CLIFF_REGION.size[2] as i32 - 1;
    let entry: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == 0).collect();
    assert!(!entry.is_empty() && !exit.is_empty());
    assert!(
        connected(&cells, &entry, &exit),
        "the path does not go anywhere"
    );

    let cut: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[0] != 0).collect();
    assert!(
        !connected(&cut, &entry, &exit),
        "with the ledge lane removed the path still connects end to end — the \
         niches are decoration beside a walkable bypass, not a knockback test"
    );

    // ...and the lane really is one cell wide: nothing stands on the backing
    // wall's column, and the only cells off the lane are the recesses.
    let niches: BTreeSet<[i32; 3]> = indexed(&out.anchors, "niche").into_iter().collect();
    for cell in &cells {
        assert!(
            cell[0] == 0 || niches.contains(cell),
            "{cell:?} is standable but is neither the ledge lane nor a declared recess"
        );
    }
}

/// A watch cell is a promise that the ambush is *legible from up the path*, and
/// the promise is checked, not asserted.
///
/// What it sees is the recess's **mouth** — the ledge cell the niche opens onto
/// — and not the recess interior. That is not an approximation: a one-deep
/// recess off a one-wide ledge is geometrically invisible from anywhere down
/// the path (the ray grazes the wall between), which is precisely what makes it
/// an ambush. The legible thing is the contested ground, and the contested
/// ground is where the player will be standing when the occupant swings.
#[test]
fn every_niche_watch_sees_the_mouth_of_its_niche() {
    let out = expand_at(&cliff_path(), CLIFF_REGION, CLIFF_SEED);
    let model = &out.model;
    let niches = indexed(&out.anchors, "niche");
    let watches = indexed(&out.anchors, "niche-watch");
    assert_eq!(niches.len(), watches.len());
    for (niche, watch) in niches.iter().zip(&watches) {
        assert_eq!(watch[0], 0, "a watch cell stands on the ledge lane");
        assert!(
            watch[2] > niche[2],
            "the watch cell must come BEFORE the niche: travel runs toward \
             local Z minimum, so {watch:?} has to be up-path of {niche:?}"
        );
        assert!(
            standable(model, *watch),
            "{watch:?} is not somewhere a player can stand"
        );
        let mouth = [0, niche[1], niche[2]];
        assert!(standable(model, mouth), "the recess mouth {mouth:?}");
        if let Err(blocker) = sees(model, *watch, mouth) {
            panic!(
                "{watch:?} cannot see the mouth {mouth:?} of {niche:?}: {blocker:?} is in the way"
            );
        }
    }
}

/// The three recess variants are a real, seeded choice — teach (a corpse prop,
/// no occupant), test (an empty recess for one), twist (two adjacent). If the
/// weights never reached the draw, the vocabulary would be one niche wearing
/// three names.
#[test]
fn the_recess_variants_are_a_seeded_choice() {
    let program = cliff_path();
    let mut shapes = BTreeSet::new();
    let mut saw_corpse = false;
    let mut saw_pair = false;
    for seed in 0..24u64 {
        let out = expand_at(&program, CLIFF_REGION, seed);
        shapes.insert(out.model.canonical_bytes());
        saw_corpse |= out
            .model
            .palette()
            .iter()
            .any(|b| b.name == "minecraft:skeleton_skull");
        let niches = indexed(&out.anchors, "niche");
        saw_pair |= niches.windows(2).any(|w| w[0][2] + 1 == w[1][2]);
    }
    assert!(shapes.len() > 4, "24 seeds gave {} shapes", shapes.len());
    assert!(saw_corpse, "the teaching variant never appeared");
    assert!(saw_pair, "the paired variant never appeared");
}

/// `spacing_min` is a control, not decoration: widening it thins the niches out.
#[test]
fn the_niche_spacing_is_a_real_control() {
    let mut wide = cliff_path();
    wide.set_param("spacing_min", 11).unwrap();
    let tight = expand_at(&cliff_path(), CLIFF_REGION, CLIFF_SEED);
    let sparse = expand_at(&wide, CLIFF_REGION, CLIFF_SEED);
    assert!(
        indexed(&sparse.anchors, "niche").len() < indexed(&tight.anchors, "niche").len(),
        "raising spacing_min did not thin the niches out"
    );
}

// ---------------------------------------------------------------------------
// O — the watch bay
// ---------------------------------------------------------------------------

/// The cells of the hazard span a body could stand on, and the bay's watch cell.
fn passage_parts(out: &Expansion) -> ([i32; 3], Vec<[i32; 3]>) {
    let watch = out
        .anchors
        .get("anchor/watch")
        .expect("the passage declares its watch bay")
        .pos;
    let gate = out
        .anchors
        .get("anchor/gate")
        .expect("the passage declares its hazard span")
        .pos;
    // The span is the run of standable cells sharing the gate anchor's Z reach;
    // `span` is a parameter, so read the geometry rather than the number.
    let span: Vec<[i32; 3]> = standable_cells(&out.model)
        .into_iter()
        .filter(|c| (c[2] - gate[2]).abs() <= 1 && c[1] == gate[1])
        .collect();
    (watch, span)
}

/// The bay is what the entry says it is: a 2×2 floor, roofed, walled on three
/// sides, and open only toward the hazard.
#[test]
fn the_bay_is_a_roofed_two_by_two_open_only_toward_the_span() {
    let out = expand_at(&watch_bay(), PASSAGE_REGION, PASSAGE_SEED);
    let model = &out.model;
    let [wx, wy, wz] = out.anchors["anchor/watch"].pos;

    for dx in 0..2 {
        for dz in 0..2 {
            let floor = [wx + dx, wy, wz + dz];
            assert!(standable(model, floor), "bay floor cell {floor:?}");
            assert!(
                solid(model, [wx + dx, wy + 2, wz + dz]),
                "the bay is roofed at {:?}",
                [wx + dx, wy + 2, wz + dz]
            );
        }
        // Open toward the span (local -Z), closed at the back.
        assert!(
            passable(model, [wx + dx, wy, wz - 1]),
            "the bay's open face looks down the passage"
        );
        assert!(
            solid(model, [wx + dx, wy, wz + 2]),
            "the bay has a back wall"
        );
    }
    for dz in 0..2 {
        assert!(solid(model, [wx - 1, wy, wz + dz]), "outer wall");
        assert!(solid(model, [wx + 2, wy, wz + dz]), "divider from the lane");
    }
    assert_eq!(
        out.anchors["anchor/watch"].facing.as_str(),
        "north",
        "the bay anchor faces the way it opens — down the passage at the span"
    );
}

/// The gate the entry exists for: from the bay you can see the span, from far
/// enough away that seeing it is worth something.
#[test]
fn the_bay_sees_the_whole_hazard_span_from_a_standoff() {
    let out = expand_at(&watch_bay(), PASSAGE_REGION, PASSAGE_SEED);
    let (watch, span) = passage_parts(&out);
    assert!(span.len() >= 3, "the span has cells to watch: {span:?}");
    assert!(
        standable(&out.model, watch),
        "the watch cell is somewhere a player can stand"
    );

    for cell in &span {
        let standoff = (0..3)
            .map(|i| (watch[i] - cell[i]).abs())
            .max()
            .unwrap_or(0);
        assert!(
            standoff >= 6,
            "the bay stands only {standoff} from {cell:?}; the entry asks for 6 \
             and DW0388 refuses under 5"
        );
        if let Err(blocker) = sees(&out.model, watch, *cell) {
            panic!("the bay cannot see the span cell {cell:?}: {blocker:?} is in the way");
        }
    }
}

/// ...and the gate has teeth. Turn the program's `obstruct` knob on — it stands
/// one pillar in the approach, in the bay's own column — and the same check
/// must go red. Without this, "the sightline is clear" is a sentence no machine
/// ever disagreed with.
#[test]
fn a_pillar_in_the_line_reds_the_sightline_gate() {
    let mut blocked = watch_bay();
    blocked.set_param("obstruct", 1).unwrap();
    let out = expand_at(&blocked, PASSAGE_REGION, PASSAGE_SEED);
    let (watch, span) = passage_parts(&out);
    assert!(!span.is_empty());

    let blind: Vec<[i32; 3]> = span
        .iter()
        .copied()
        .filter(|cell| sees(&out.model, watch, *cell).is_err())
        .collect();
    assert!(
        !blind.is_empty(),
        "a pillar was stood in the bay's line of sight and the check still \
         reported every span cell visible — the gate proves nothing"
    );
    // The obstruction is a pillar, not a sealed wall: the passage is still
    // walkable, so what the check caught is blindness and not impassability.
    let cells = standable_cells(&out.model);
    let far = PASSAGE_REGION.size[2] as i32 - 1;
    let entry: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == 0).collect();
    assert!(connected(&cells, &entry, &exit));
}

/// The standoff is enforced by the rule, not by the caller remembering it: a
/// program whose approach is shorter than the entry's six blocks has no
/// applicable alternative and says so.
#[test]
fn an_approach_under_the_standoff_is_refused_not_shortened() {
    let mut cramped = watch_bay();
    cramped.set_param("approach", 4).unwrap();
    let err = expand(
        &cramped,
        PASSAGE_REGION,
        &ExpandOptions::seeded(PASSAGE_SEED),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected a refusal, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// R — the rafter perch
// ---------------------------------------------------------------------------

/// Every cell of the model at the same height as `anchor/perch-1`, which is the
/// perch course: the layer the truss's standable cells live in.
fn perch_course_y(anchors: &BTreeMap<String, Anchor>) -> i32 {
    indexed(anchors, "perch")
        .first()
        .expect("the trussed hall declares perches")[1]
}

/// The fixture: seven rafters over a nave, byte-identical on a second
/// expansion, anchors and all.
#[test]
fn a_rafter_hall_expands_deterministically() {
    let program = rafter_hall();
    let a = expand_at(&program, HALL_REGION, HALL_SEED);
    let b = expand_at(&program, HALL_REGION, HALL_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);

    let perches = indexed(&a.anchors, "perch");
    assert_eq!(perches.len(), 7, "the pinned fixture is a seven-perch hall");
    assert!(
        a.anchors.contains_key("anchor/hall-door"),
        "the hall names the cell its fairness is measured from: {:#?}",
        a.anchors
    );
    // Sides alternate, so the truss is not a row of shelves down one wall — the
    // dossier's monoculture complaint applies within a rule as well as across a
    // level.
    let xs: BTreeSet<i32> = perches.iter().map(|p| p[0]).collect();
    assert_eq!(xs.len(), 2, "perches sit on both walls: {perches:?}");
}

/// The gate the entry exists for: standing in the doorway, you can see every
/// rafter. Fairness in the souls grammar is carried by silhouette
/// (`docs/notes/souls-design-language.md` §4.3), and a silhouette you cannot
/// see is not a telegraph.
#[test]
fn every_perch_is_visible_from_the_hall_door() {
    let out = expand_at(&rafter_hall(), HALL_REGION, HALL_SEED);
    let model = &out.model;
    let door = out.anchors["anchor/hall-door"].pos;
    assert!(standable(model, door), "the door cell {door:?}");

    let perches = indexed(&out.anchors, "perch");
    assert!(!perches.is_empty());
    for perch in &perches {
        if let Err(blocker) = sees(model, door, *perch) {
            panic!(
                "the doorway {door:?} cannot see the perch {perch:?}: {blocker:?} is in the way \
                 — a rafter the player cannot read from the door is an ambush with no telegraph"
            );
        }
    }
}

/// ...and the gate has teeth. `span_beams = 1` builds the *obvious* truss —
/// timbers spanning wall to wall — and the same check must go red, because an
/// eye on the floor is below the beam plane and a perch is above it, so the ray
/// has to cross the plane and past a few cells of hall some nearer beam is
/// always in the crossing. That is the whole reason this rule's rafters are
/// corbels with an open centre span.
#[test]
fn a_full_span_truss_blinds_the_perches() {
    let mut blinded = rafter_hall();
    blinded.set_param("span_beams", 1).unwrap();
    let out = expand_at(&blinded, HALL_REGION, HALL_SEED);
    let model = &out.model;
    let door = out.anchors["anchor/hall-door"].pos;

    let blind: Vec<[i32; 3]> = indexed(&out.anchors, "perch")
        .into_iter()
        .filter(|p| sees(model, door, *p).is_err())
        .collect();
    assert!(
        !blind.is_empty(),
        "the truss was closed across the nave and the doorway still saw every \
         perch — the sightline gate proves nothing"
    );

    // The timbers are a ceiling, not a wall: the nave stays walkable end to end,
    // so what the gate caught is blindness and not a severed hall.
    let cells = standable_cells(model);
    let far = HALL_REGION.size[2] as i32 - 1;
    let entry: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == 0).collect();
    assert!(connected(&cells, &entry, &exit));
}

/// The second gate: at most one perch per 24 floor cells. The Cathedral's
/// documented failure is ambush *monoculture*, and a density cap is the smallest
/// machine-checkable form of "not every space is the same trick".
#[test]
fn the_perch_density_stays_under_the_cap() {
    let out = expand_at(&rafter_hall(), HALL_REGION, HALL_SEED);
    let perches = indexed(&out.anchors, "perch").len() as i64;
    let floor = nave_floor_cells(&out);
    assert!(
        perches * FLOOR_CELLS_PER_PERCH <= floor,
        "{perches} perches over {floor} floor cells is denser than one per \
         {FLOOR_CELLS_PER_PERCH}"
    );

    // ...and the cap is what the rule refuses on, not something the fixture
    // happens to satisfy. Rafter spacing runs *along* the hall, so a narrower
    // hall of the same length carries the same seven rafters over less floor.
    // Eight across is the width at which that genuinely breaks the cap — and it
    // is the width at which the rule declines to build.
    const NARROW_WIDTH: u32 = 8;
    let narrow = Box3::at_origin([NARROW_WIDTH, 6, HALL_REGION.size[2]]);
    let narrow_floor = (NARROW_WIDTH as i64 - 2) * HALL_REGION.size[2] as i64;
    assert!(
        perches * FLOOR_CELLS_PER_PERCH > narrow_floor,
        "the narrower hall would not have broken the cap ({perches} perches over \
         {narrow_floor} floor cells), so refusing it proves nothing"
    );
    let err = expand(&rafter_hall(), narrow, &ExpandOptions::seeded(HALL_SEED)).unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected a refusal on the density cap, got: {err}"
    );
}

/// The third gate: a rafter is geometry, not a coordinate. Every perch is a cell
/// a body stands in, on timber, with headroom — and so is the rest of the corbel
/// it sits on, because an occupant that can only exist on one cell is a spawn
/// point wearing a beam's name.
#[test]
fn the_rafters_are_geometry_a_body_can_stand_on() {
    let out = expand_at(&rafter_hall(), HALL_REGION, HALL_SEED);
    let model = &out.model;
    let perches = indexed(&out.anchors, "perch");
    assert!(!perches.is_empty());
    for perch in &perches {
        let [x, y, z] = *perch;
        assert!(standable(model, *perch), "the perch cell {perch:?}");
        assert!(
            solid(model, [x, y - 1, z]),
            "{:?} is not timber — the perch has nothing under it",
            [x, y - 1, z]
        );
        // The beam runs on into the wall, so there is a rafter to walk out along.
        let inward = if x < HALL_REGION.size[0] as i32 / 2 {
            -1
        } else {
            1
        };
        let along = [x + inward, y, z];
        assert!(
            standable(model, along),
            "{along:?}, the next cell of the same beam, is not standable"
        );
    }

    // The red side: the perch course is *mostly* not standable. If it were, the
    // assertion above would hold over any hall with a floor at that height and
    // would be saying nothing about rafters at all.
    let y = perch_course_y(&out.anchors);
    let mid = HALL_REGION.size[0] as i32 / 2;
    assert!(
        !standable(model, [mid, y, perches[0][2]]),
        "the centre of the nave is standable at rafter height — the truss is not \
         a truss, it is a floor"
    );

    // And a rafter is not a mezzanine: nothing walks up to it from the ground.
    let cells = standable_cells(model);
    let floor: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[1] < y).collect();
    let up: BTreeSet<[i32; 3]> = perches.iter().copied().collect();
    assert!(
        !connected(&cells, &floor, &up),
        "a perch is reachable on foot from the nave — that is a ledge, not a rafter"
    );
}

/// A hall too short for the truss layer emits none, and is still a hall: same
/// shell, same door anchor, nothing missing and nothing refused. Both shapes are
/// asserted, because "optional" is a claim about two outputs.
#[test]
fn a_hall_under_six_tall_is_a_hall_without_rafters() {
    let tall = expand_at(&rafter_hall(), HALL_REGION, HALL_SEED);
    let short = expand_at(&rafter_hall(), SHORT_HALL_REGION, HALL_SEED);

    assert!(!indexed(&tall.anchors, "perch").is_empty());
    assert!(
        indexed(&short.anchors, "perch").is_empty(),
        "a five-tall hall grew rafters: {:#?}",
        short.anchors
    );
    assert!(
        short.anchors.contains_key("anchor/hall-door"),
        "the short hall still names its doorway"
    );
    // It is a room, not a solid block: the nave is walkable end to end.
    let cells = standable_cells(&short.model);
    let far = SHORT_HALL_REGION.size[2] as i32 - 1;
    let entry: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == 0).collect();
    assert!(!entry.is_empty());
    assert!(connected(&cells, &entry, &exit));
}

/// Floor cells of the nave — the denominator the density cap is stated over.
/// Measured off the model rather than computed from the region, because the side
/// walls are not floor anybody stands on.
fn nave_floor_cells(out: &Expansion) -> i64 {
    let region = out.model.region();
    let y = region.origin[1] + 1;
    region
        .positions()
        .filter(|&p| p[1] == y && standable(&out.model, p))
        .count() as i64
}

// ---------------------------------------------------------------------------
// A — the corner-ambush alcove
// ---------------------------------------------------------------------------

/// The wall's Z, read off the geometry rather than recomputed from parameters:
/// the one plane between the alcove and the approach that is solid everywhere
/// except the doorway.
fn wall_plane(out: &Expansion) -> i32 {
    let alcove = out.anchors["anchor/alcove"].pos;
    let threshold = out.anchors["anchor/threshold"].pos;
    assert_eq!(
        alcove[1], threshold[1],
        "the alcove and the doorway share a floor"
    );
    threshold[2]
}

/// The gate this entry exists for, and the first one in the vocabulary that runs
/// backwards: from **no** standable cell of the approach can the alcove be seen.
/// Asserted cell by cell — a single counter-example is the ambush stopping being
/// one.
#[test]
fn the_alcove_is_blind_from_every_approach_cell() {
    let out = expand_at(&ambush_door(), DOOR_REGION, DOOR_SEED);
    let model = &out.model;
    let alcove = out.anchors["anchor/alcove"].pos;
    let wall = wall_plane(&out);
    assert!(standable(model, alcove), "the alcove cell {alcove:?}");

    let approach: Vec<[i32; 3]> = standable_cells(model)
        .into_iter()
        .filter(|c| c[2] > wall)
        .collect();
    assert!(
        approach.len() > 20,
        "the fixture has an approach worth checking: {}",
        approach.len()
    );
    for cell in &approach {
        if sees(model, *cell, alcove).is_ok() {
            panic!(
                "the alcove {alcove:?} is visible from the approach cell {cell:?} — a corner \
                 ambush the player can read through the door is a corner ambush that will not \
                 happen"
            );
        }
    }
}

/// ...and the gate has teeth. `expose = 1` widens the opening over the alcove's
/// own lane, which is the one mistake that turns the pocket into a lit stage,
/// and the same check must go red.
#[test]
fn a_widened_doorway_exposes_the_alcove() {
    let mut exposed = ambush_door();
    exposed.set_param("expose", 1).unwrap();
    let out = expand_at(&exposed, DOOR_REGION, DOOR_SEED);
    let alcove = out.anchors["anchor/alcove"].pos;
    let wall = wall_plane(&out);

    let seen: Vec<[i32; 3]> = standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[2] > wall && sees(&out.model, *c, alcove).is_ok())
        .collect();
    assert!(
        !seen.is_empty(),
        "the doorway was widened straight over the alcove and the blindness check \
         still reported it hidden from every approach cell — the gate proves nothing"
    );
}

/// The second gate: the alcove is *one swing* from the cell the player lands in.
/// A blind pocket three cells away is a room the ambusher has to cross, which is
/// a different (and much weaker) encounter.
#[test]
fn the_alcove_is_one_swing_from_the_doorways_inside_cell() {
    // Swept over `door_offset`, because an adjacency that only holds at the
    // default is an adjacency nobody arranged.
    for offset in [1i64, 2, 4] {
        let mut program = ambush_door();
        program.set_param("door_offset", offset).unwrap();
        let out = expand_at(&program, DOOR_REGION, DOOR_SEED);
        let model = &out.model;
        let alcove = out.anchors["anchor/alcove"].pos;
        let threshold = out.anchors["anchor/threshold"].pos;
        let inside = [threshold[0], threshold[1], threshold[2] - 1];

        assert!(
            standable(model, inside),
            "offset {offset}: the cell inside the doorway {inside:?}"
        );
        let step = (0..3).map(|i| (alcove[i] - inside[i]).abs()).sum::<i32>();
        assert_eq!(
            step, 1,
            "offset {offset}: the alcove {alcove:?} is not adjacent to the doorway's \
             inside cell {inside:?}"
        );
        assert_ne!(alcove[2], threshold[2], "the alcove is not in the wall");
        // The anchor really did move with the door — the fixture is not agreeing
        // with two hard-coded numbers that happen to differ by one.
        assert_eq!(threshold[0], offset as i32 + 1);
    }
}

/// The third gate, the same shape as `cliff_path`'s: the doorway is the **only**
/// way through. A wall with a second hole in it is scenery, and an ambush beside
/// scenery is optional.
#[test]
fn the_doorway_is_the_only_route_through_the_wall() {
    let out = expand_at(&ambush_door(), DOOR_REGION, DOOR_SEED);
    let cells = standable_cells(&out.model);
    let wall = wall_plane(&out);
    let threshold = out.anchors["anchor/threshold"].pos;

    let approach: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] > wall).collect();
    let inside: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] < wall).collect();
    assert!(!approach.is_empty() && !inside.is_empty());
    assert!(
        connected(&cells, &approach, &inside),
        "the doorway does not go anywhere"
    );

    let cut: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[0] != threshold[0] || c[2] != wall)
        .collect();
    assert!(
        !connected(&cut, &approach, &inside),
        "with the doorway plugged the wall still lets a walker through — there is \
         a second way round, so passing the alcove is optional"
    );
}

// ---------------------------------------------------------------------------
// C — the container tell
// ---------------------------------------------------------------------------

/// The odd barrel's block, and the plain one's.
const TELL_BLOCK: &str = "minecraft:spruce_log";
const BARREL_BLOCK: &str = "minecraft:barrel";

/// The first gate: **exactly** one tell, every time, and the anchor is on it.
/// The count is read off the blocks, not off the anchors — an anchor that names
/// a plain barrel would satisfy any assertion about anchors alone.
#[test]
fn the_barrel_line_holds_exactly_one_tell() {
    let program = store_room();
    for seed in 0..12u64 {
        let out = expand_at(&program, STORE_REGION, seed);
        let model = &out.model;
        let tells: Vec<[i32; 3]> = STORE_REGION
            .positions()
            .filter(|&p| model.get(p).is_some_and(|b| b.name.starts_with(TELL_BLOCK)))
            .collect();
        assert_eq!(tells.len(), 1, "seed {seed} laid {tells:?}");

        let anchor = out.anchors["anchor/tell"].pos;
        assert_eq!(anchor, tells[0], "seed {seed}: the anchor is off the tell");

        // ...and it is one *among* barrels: the rest of the row is the plain
        // variant, so "exactly one" is not because the row is one cell long.
        let barrels: Vec<[i32; 3]> = STORE_REGION
            .positions()
            .filter(|&p| {
                model
                    .get(p)
                    .is_some_and(|b| b.name.starts_with(BARREL_BLOCK))
            })
            .collect();
        assert_eq!(
            barrels.len(),
            STORE_REGION.size[2] as usize - 1,
            "seed {seed}: the row is not full of plain barrels"
        );
    }
}

/// The second gate: the tell is *in* the line — a barrel beside it on at least
/// one side. The mimic's tell works because the chest is standing where chests
/// stand (`docs/notes/souls-design-language.md` §2.3); one odd barrel off on its
/// own is a prop, not a tell.
#[test]
fn the_tell_stands_in_the_line_it_is_odd_against() {
    let program = store_room();
    for seed in 0..12u64 {
        let out = expand_at(&program, STORE_REGION, seed);
        let model = &out.model;
        let [x, y, z] = out.anchors["anchor/tell"].pos;
        let neighbours = [[x, y, z - 1], [x, y, z + 1]]
            .into_iter()
            .filter(|&p| {
                model
                    .get(p)
                    .is_some_and(|b| b.name.starts_with(BARREL_BLOCK))
            })
            .count();
        assert!(
            neighbours >= 1,
            "seed {seed}: the tell at {:?} has no barrel beside it",
            [x, y, z]
        );
        // The line runs the whole lane, so the tell is inside a row and not at
        // the end of a stub the rule stopped building.
        let line_start = out.anchors["anchor/store-line"].pos;
        assert_eq!([line_start[0], line_start[1]], [x, y]);
        assert_eq!(
            line_start[2],
            STORE_REGION.size[2] as i32 - 1,
            "the line's near end is the approach end"
        );
    }
}

/// The tell's position is the seed's, not the rule's: twelve seeds must not all
/// put it in the same place, or the "ambush tell" is a fixed landmark players
/// learn once and never look for again.
#[test]
fn the_tell_moves_with_the_seed() {
    let program = store_room();
    let places: BTreeSet<[i32; 3]> = (0..12u64)
        .map(|seed| expand_at(&program, STORE_REGION, seed).anchors["anchor/tell"].pos)
        .collect();
    assert!(
        places.len() >= 3,
        "12 seeds put the tell in {} places: {places:?}",
        places.len()
    );
}

// ---------------------------------------------------------------------------
// W — the worn-tread tell, and S — the side pockets
// ---------------------------------------------------------------------------

/// The fixture: a four-pocket lane, byte-identical on a second expansion,
/// anchors and all.
#[test]
fn a_worn_tread_lane_expands_deterministically() {
    let program = boulder_stair();
    let a = expand_at(&program, STAIR_REGION, STAIR_SEED);
    let b = expand_at(&program, STAIR_REGION, STAIR_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);

    let pockets = indexed(&a.anchors, "pocket");
    assert_eq!(pockets.len(), 4, "the pinned fixture is a four-pocket lane");
    assert!(a.anchors.contains_key("anchor/stair-run"));
    assert!(a.anchors.contains_key("anchor/volley-slot"));
}

/// The gate the entry exists for, read off the geometry: the run's own floor
/// column is the `smooth` block at every depth, and every other floor cell in
/// the lane (both rough side lanes, the pocket band when it is not a niche,
/// and the backing) is the `rough` block. This is the claim the palette
/// mirror below has to stay silent about.
#[test]
fn the_run_is_smooth_and_every_other_floor_cell_is_rough() {
    let out = expand_at(&boulder_stair(), STAIR_REGION, STAIR_SEED);
    let model = &out.model;
    let run = out.anchors["anchor/stair-run"].pos;
    let region = model.region();
    for x in region.origin[0]..region.maximum()[0] {
        for z in region.origin[2]..region.maximum()[2] {
            let block = model.get([x, run[1], z]).expect("a floor cell");
            let want = if x == run[0] {
                "minecraft:stone"
            } else {
                "minecraft:cobblestone"
            };
            assert_eq!(
                block.name,
                want,
                "floor cell {:?} is not the expected worn-tread material",
                [x, run[1], z]
            );
        }
    }
}

/// The volley anchor sits in the vault directly over the run: same `X`, same
/// `Z`, `head` + 1 above the tread.
#[test]
fn the_volley_slot_sits_in_the_vault_over_the_run() {
    let out = expand_at(&boulder_stair(), STAIR_REGION, STAIR_SEED);
    let run = out.anchors["anchor/stair-run"].pos;
    let volley = out.anchors["anchor/volley-slot"].pos;
    assert_eq!(
        volley[0], run[0],
        "the rib sits over the run, not beside it"
    );
    assert_eq!(volley[2], run[2]);
    assert_eq!(volley[1] - run[1], 5, "head (4) + 1 above the tread");
    assert!(
        solid(&out.model, volley),
        "the vault rib is ordinary stone until a campaign binds the anchor"
    );
}

/// The hazard lane — far side, run, near side — is the only continuous route
/// down the box. The pocket band is not a bypass: it is solid everywhere
/// except at pocket slots, which do not run the lane's whole length.
#[test]
fn the_hazard_lane_is_the_only_continuous_route() {
    let out = expand_at(&boulder_stair(), STAIR_REGION, STAIR_SEED);
    let cells = standable_cells(&out.model);
    let far = STAIR_REGION.size[2] as i32 - 1;
    let entry: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == 0).collect();
    assert!(!entry.is_empty() && !exit.is_empty());
    assert!(
        connected(&cells, &entry, &exit),
        "the lane does not go anywhere"
    );

    let pocket_x = indexed(&out.anchors, "pocket")[0][0];
    let cut: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[0] == pocket_x).collect();
    assert!(
        !connected(&cut, &entry, &exit),
        "the pocket band alone connects the box end to end — it is a parallel \
         bypass, not a side dodge off a lane the player has to take"
    );
}

/// Every pocket is a one-cell dodge: standable, backed by solid stone so the
/// notch never opens onto the model's own edge, lintelled so it reads as a
/// niche and not a doorway — and, unlike `ambush_door`'s alcove, visible from
/// the lane cell right beside it. A dodge nobody can see coming is not an
/// escape.
#[test]
fn every_pocket_is_a_one_cell_dodge_visible_from_the_lane() {
    let out = expand_at(&boulder_stair(), STAIR_REGION, STAIR_SEED);
    let model = &out.model;
    let pockets = indexed(&out.anchors, "pocket");
    assert!(!pockets.is_empty());
    for pos in pockets {
        let [x, y, z] = pos;
        assert!(standable(model, pos), "the pocket cell {pos:?}");
        assert!(
            solid(model, [x + 1, y, z]),
            "the pocket is exactly one deep — {:?} must be backing wall",
            [x + 1, y, z]
        );
        assert!(solid(model, [x, y - 1, z]), "the pocket has a floor");
        assert!(
            solid(model, [x, y + 2, z]),
            "the pocket has a lintel at pocket_height (2)"
        );
        let lane_cell = [x - 1, y, z];
        assert!(passable(model, lane_cell), "the near lane beside {pos:?}");
        if let Err(blocker) = sees(model, lane_cell, pos) {
            panic!(
                "{lane_cell:?} cannot see the pocket {pos:?} it opens onto: {blocker:?} is in \
                 the way — a dodge nobody can see coming is not an escape"
            );
        }
    }
}

/// `pocket_period` is a real control: widening it thins the pockets out — the
/// same claim `cliff_path` makes for `spacing_min`.
#[test]
fn the_pocket_period_is_a_real_control() {
    let mut sparse = boulder_stair();
    sparse.set_param("pocket_period", 20).unwrap();
    let tight = expand_at(&boulder_stair(), STAIR_REGION, STAIR_SEED);
    let wide = expand_at(&sparse, STAIR_REGION, STAIR_SEED);
    assert!(
        indexed(&wide.anchors, "pocket").len() < indexed(&tight.anchors, "pocket").len(),
        "raising pocket_period did not thin the pockets out"
    );
}

/// A box shorter than one `pocket_period` cannot tile even one pocket
/// (`make_split` checks the un-repeated pattern before it tiles) — and that
/// is a variant, not an error: the same shape `rafter_hall` uses for a hall
/// too short for its truss. Both shapes are asserted, because "optional" is a
/// claim about two outputs.
#[test]
fn a_lane_too_short_for_one_pocket_period_is_a_lane_without_pockets() {
    let long = expand_at(&boulder_stair(), STAIR_REGION, STAIR_SEED);
    let short = expand_at(&boulder_stair(), SHORT_STAIR_REGION, STAIR_SEED);

    assert!(!indexed(&long.anchors, "pocket").is_empty());
    assert!(
        indexed(&short.anchors, "pocket").is_empty(),
        "a lane shorter than pocket_period grew pockets: {:#?}",
        short.anchors
    );
    assert!(short.anchors.contains_key("anchor/stair-run"));
    let cells = standable_cells(&short.model);
    let far = SHORT_STAIR_REGION.size[2] as i32 - 1;
    let entry: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == 0).collect();
    assert!(!entry.is_empty());
    assert!(connected(&cells, &entry, &exit));
}

// ---------------------------------------------------------------------------
// The spec-0027 §4 palette-role budget — a test-local mirror
//
// The craft diagnostics are a later phase of spec-0027
// (`docs/reference/grammar.md` §7; `crates/grammar/src/lib.rs`'s own "not
// built yet" note): there is no `DW`-numbered palette-role-budget check to
// call. What follows is a small reimplementation of the *described* rule
// (60/30/10, accent < 10%, grouped by material family) — the same move
// `watch_bay`'s sightline gate already makes against `DW0388`. It mints no
// diagnostic code; that stays planner-owned.
// ---------------------------------------------------------------------------

/// A family is an "accent" once it exceeds this share and is not the
/// dominant family present — spec-0027 §4's own number.
const ACCENT_CEILING: f64 = 0.10;

/// Family shares among the filled cells `cells` names — deliberately scoped
/// by the caller to the band a rule's palette claim is actually about (a
/// lane's own floor course, a grate row's own cells), so incidental
/// structural stone elsewhere in the model does not dilute a claim that is
/// specifically about that band. `families` maps a family label to the block
/// names that belong to it; any block named by no family is its own
/// singleton family.
fn family_shares(
    model: &VoxelModel,
    cells: impl Iterator<Item = [i32; 3]>,
    families: &[(&str, &[&str])],
) -> BTreeMap<String, f64> {
    let mut counts: BTreeMap<String, u64> = BTreeMap::new();
    let mut total: u64 = 0;
    for pos in cells {
        let Some(block) = model.get(pos) else {
            continue;
        };
        if block.is_air() {
            continue;
        }
        total += 1;
        let family = families
            .iter()
            .find(|(_, names)| names.contains(&block.name.as_str()))
            .map(|(label, _)| (*label).to_string())
            .unwrap_or_else(|| block.name.clone());
        *counts.entry(family).or_insert(0) += 1;
    }
    let total = total.max(1);
    counts
        .into_iter()
        .map(|(family, n)| (family, n as f64 / total as f64))
        .collect()
}

/// Families that are neither the dominant one present nor under the accent
/// ceiling — the spec-0027 §4 violation this mirror is standing in for.
fn accent_overruns(shares: &BTreeMap<String, f64>) -> Vec<(String, f64)> {
    let dominant = shares
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(k, _)| k.clone());
    shares
        .iter()
        .filter(|(k, s)| Some((*k).clone()) != dominant && **s > ACCENT_CEILING)
        .map(|(k, &v)| (k.clone(), v))
        .collect()
}

/// Cells of the lane's own floor course at `y` (the run's floor level).
fn stair_floor_cells(region: Box3, y: i32) -> impl Iterator<Item = [i32; 3]> {
    (region.origin[0]..region.maximum()[0])
        .flat_map(move |x| (region.origin[2]..region.maximum()[2]).map(move |z| [x, y, z]))
}

/// The claim `boulder_stair`'s doc-comment makes: the smooth run is the same
/// material family as the rough lane, at a different distress level, so it
/// must not register as an accent. Proved both ways — grouped by family it
/// is silent, and *because* an ungrouped reading of the same cells would
/// genuinely trip the accent ceiling, the fold is load-bearing rather than
/// vacuous.
#[test]
fn the_worn_tread_variant_is_not_counted_as_an_accent() {
    let out = expand_at(&boulder_stair(), STAIR_PALETTE_REGION, STAIR_SEED);
    let run_y = out.anchors["anchor/stair-run"].pos[1];
    let cells: Vec<[i32; 3]> = stair_floor_cells(STAIR_PALETTE_REGION, run_y).collect();

    let grouped = family_shares(
        &out.model,
        cells.iter().copied(),
        &[("rock", &["minecraft:cobblestone", "minecraft:stone"])],
    );
    assert_eq!(
        grouped.len(),
        1,
        "grouped by family there is exactly one material on the tread: {grouped:?}"
    );
    assert!(
        accent_overruns(&grouped).is_empty(),
        "the family-grouped tread was flagged as carrying an accent: {grouped:?}"
    );

    // The fold matters: read the *same* cells without it and the smooth run
    // is a distinct role whose share clears the accent ceiling on its own.
    let naive = family_shares(&out.model, cells.iter().copied(), &[]);
    let smooth_share = naive.get("minecraft:stone").copied().unwrap_or(0.0);
    assert!(
        smooth_share > ACCENT_CEILING,
        "the fixture does not give the naive reading real teeth: smooth is only \
         {smooth_share:.3} of the tread"
    );
    assert!(
        !accent_overruns(&naive).is_empty(),
        "an ungrouped reading of the same cells did not trip the accent ceiling — the family \
         fold above is not proving anything"
    );
}

/// ...and the family-grouped reading is not simply blind: restyle the run to
/// a wholly unrelated material and it correctly reads as a genuine accent
/// overrun, over the same cells, with the same family map.
#[test]
fn the_palette_mirror_still_fires_on_a_genuine_accent_in_the_tread() {
    let mut restyled = boulder_stair();
    restyled
        .set_role(
            "smooth",
            delvewright_grammar::ir::Paint::Block(BlockState::simple("gold_block")),
        )
        .unwrap();
    let out = expand_at(&restyled, STAIR_PALETTE_REGION, STAIR_SEED);
    let run_y = out.anchors["anchor/stair-run"].pos[1];
    let cells: Vec<[i32; 3]> = stair_floor_cells(STAIR_PALETTE_REGION, run_y).collect();

    let grouped = family_shares(
        &out.model,
        cells.iter().copied(),
        &[("rock", &["minecraft:cobblestone", "minecraft:stone"])],
    );
    let overruns = accent_overruns(&grouped);
    assert!(
        overruns.iter().any(|(k, _)| k == "minecraft:gold_block"),
        "a genuinely unrelated material was not caught as an accent overrun: {grouped:?}"
    );
}

// ---------------------------------------------------------------------------
// M — the threshold motif
// ---------------------------------------------------------------------------

/// The fixture: a curtained doorway, byte-identical on a second expansion.
#[test]
fn a_threshold_motif_expands_deterministically() {
    let program = threshold_motif();
    let a = expand_at(&program, THRESHOLD_REGION, THRESHOLD_SEED);
    let b = expand_at(&program, THRESHOLD_REGION, THRESHOLD_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
    assert!(a.anchors.contains_key("anchor/threshold-narrate"));
}

/// The narrate anchor re-centres itself at whatever width the box is built —
/// the same formula holding at three different sizes is what "reusable at
/// different box sizes" means for a *point* anchor.
#[test]
fn the_narrate_anchor_recentres_at_different_widths() {
    for region in [
        Box3::at_origin([5, 6, 9]),
        THRESHOLD_REGION,
        WIDE_THRESHOLD_REGION,
    ] {
        let out = expand_at(&threshold_motif(), region, THRESHOLD_SEED);
        let narrate = out.anchors["anchor/threshold-narrate"].pos;
        assert_eq!(
            narrate[0],
            (region.size[0] as i32 - 1) / 2,
            "the anchor is not centred on a {region:?} doorway"
        );
        // The anchor marks the floor block itself (`FloorCenter`'s body is
        // `fill("stone")`), so the standable cell is the one above it.
        assert!(standable(
            &out.model,
            [narrate[0], narrate[1] + 1, narrate[2]]
        ));
    }
}

/// Curtain strands in the doorway: the first `y` above the narrate anchor's
/// floor that carries any `chain` block is the curtain band, and the count
/// is how many distinct `x` columns carry one there.
fn curtain_strand_count(out: &Expansion, region: Box3) -> usize {
    let narrate = out.anchors["anchor/threshold-narrate"].pos;
    let y = (narrate[1] + 1..region.maximum()[1])
        .find(|&y| {
            (region.origin[0]..region.maximum()[0]).any(|x| {
                out.model
                    .get([x, y, narrate[2]])
                    .is_some_and(|b| b.name == "minecraft:chain")
            })
        })
        .expect("the doorband has a curtain band somewhere above the floor");
    (region.origin[0]..region.maximum()[0])
        .filter(|&x| {
            out.model
                .get([x, y, narrate[2]])
                .is_some_and(|b| b.name == "minecraft:chain")
        })
        .count()
}

/// The gate the entry exists for: strand density holds across box sizes —
/// widening the doorway does not thin the curtain out, because the band is a
/// `split_repeat` tiling the same period regardless of width.
#[test]
fn curtain_density_holds_across_box_sizes() {
    let narrow = expand_at(&threshold_motif(), THRESHOLD_REGION, THRESHOLD_SEED);
    let wide = expand_at(&threshold_motif(), WIDE_THRESHOLD_REGION, THRESHOLD_SEED);
    let narrow_strands = curtain_strand_count(&narrow, THRESHOLD_REGION);
    let wide_strands = curtain_strand_count(&wide, WIDE_THRESHOLD_REGION);
    assert!(narrow_strands >= 6, "{narrow_strands}");
    assert!(
        wide_strands > narrow_strands,
        "the wider doorway did not carry proportionally more strands: \
         {narrow_strands} at {THRESHOLD_REGION:?} vs {wide_strands} at {WIDE_THRESHOLD_REGION:?}"
    );
    let narrow_density = narrow_strands as f64 / (THRESHOLD_REGION.size[0] - 2) as f64;
    let wide_density = wide_strands as f64 / (WIDE_THRESHOLD_REGION.size[0] - 2) as f64;
    assert!(
        (narrow_density - wide_density).abs() < 0.01,
        "the strand density drifted with width: {narrow_density:.3} vs {wide_density:.3} — the \
         motif degraded rather than scaled"
    );
}

/// ...and the gate has teeth: `single_strand` collapses the curtain to one
/// strand regardless of width, which is exactly what "the motif degrading"
/// means, and the density check above must have been able to catch it.
#[test]
fn single_strand_degrades_the_curtain_at_width() {
    let mut collapsed = threshold_motif();
    collapsed.set_param("single_strand", 1).unwrap();
    let narrow = expand_at(&collapsed, THRESHOLD_REGION, THRESHOLD_SEED);
    let wide = expand_at(&collapsed, WIDE_THRESHOLD_REGION, THRESHOLD_SEED);
    let narrow_strands = curtain_strand_count(&narrow, THRESHOLD_REGION);
    let wide_strands = curtain_strand_count(&wide, WIDE_THRESHOLD_REGION);
    assert_eq!(narrow_strands, 1);
    assert_eq!(
        wide_strands, 1,
        "a wider doorway grew more strands even under single_strand — the knob proves nothing"
    );
}

/// The curtain hangs above the walk clearance and never touches it: the
/// doorway stays walkable end to end, with or without the degrading knob.
#[test]
fn the_doorway_is_walkable_beneath_the_curtain() {
    for single_strand in [0, 1] {
        let mut program = threshold_motif();
        program.set_param("single_strand", single_strand).unwrap();
        let out = expand_at(&program, THRESHOLD_REGION, THRESHOLD_SEED);
        let cells = standable_cells(&out.model);
        let far = THRESHOLD_REGION.size[2] as i32 - 1;
        let entry: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == far).collect();
        let exit: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] == 0).collect();
        assert!(!entry.is_empty() && !exit.is_empty());
        assert!(
            connected(&cells, &entry, &exit),
            "single_strand={single_strand}: the curtain blocked the doorway"
        );
    }
}

// ---------------------------------------------------------------------------
// X — the broken grate
// ---------------------------------------------------------------------------

const GRATE_BLOCK: &str = "minecraft:iron_bars";
const BROKEN_BLOCK: &str = "minecraft:mossy_cobblestone";

/// The first gate, the same shape as `store_room`'s: **exactly** one break,
/// every time, and the anchor is on it. Counted off the blocks' `(x, z)`, not
/// raw block count — a break is `grate_height` (2) courses tall, so it is
/// two blocks at one row position, not two breaks — and the rest of the row
/// is asserted to be plain grates.
#[test]
fn the_grate_wall_holds_exactly_one_break() {
    let program = broken_grate();
    for seed in 0..12u64 {
        let out = expand_at(&program, GRATE_REGION, seed);
        let model = &out.model;
        let broken: BTreeSet<[i32; 2]> = GRATE_REGION
            .positions()
            .filter(|&p| model.get(p).is_some_and(|b| b.name == BROKEN_BLOCK))
            .map(|p| [p[0], p[2]])
            .collect();
        assert_eq!(
            broken.len(),
            1,
            "seed {seed} laid broken cells at {broken:?}"
        );

        let anchor = out.anchors["anchor/grate-secret"].pos;
        assert_eq!(
            [anchor[0], anchor[2]],
            *broken.iter().next().unwrap(),
            "seed {seed}: the anchor is off the break"
        );

        let grates: Vec<[i32; 3]> = GRATE_REGION
            .positions()
            .filter(|&p| model.get(p).is_some_and(|b| b.name == GRATE_BLOCK))
            .collect();
        // The row is `grate_height` (2) courses tall over `Z` cells of run.
        assert_eq!(
            grates.len(),
            (GRATE_REGION.size[2] as usize - 1) * 2,
            "seed {seed}: the row is not full of plain grates"
        );
    }
}

/// The second gate: the break is *in* the row, a plain grate beside it on at
/// least one side — an odd cell off on its own is a prop, not a tell.
#[test]
fn the_break_stands_in_the_row_it_is_odd_against() {
    let program = broken_grate();
    for seed in 0..12u64 {
        let out = expand_at(&program, GRATE_REGION, seed);
        let model = &out.model;
        let [x, y, z] = out.anchors["anchor/grate-secret"].pos;
        let neighbours = [[x, y, z - 1], [x, y, z + 1]]
            .into_iter()
            .filter(|&p| model.get(p).is_some_and(|b| b.name == GRATE_BLOCK))
            .count();
        assert!(
            neighbours >= 1,
            "seed {seed}: the break at {:?} has no grate beside it",
            [x, y, z]
        );
    }
}

/// The break's position is the seed's: twelve seeds must not all put it in
/// the same place, or the cue is a fixed landmark players learn once.
#[test]
fn the_break_moves_with_the_seed() {
    let program = broken_grate();
    let places: BTreeSet<[i32; 3]> = (0..12u64)
        .map(|seed| expand_at(&program, GRATE_REGION, seed).anchors["anchor/grate-secret"].pos)
        .collect();
    assert!(
        places.len() >= 3,
        "12 seeds put the break in {} places: {places:?}",
        places.len()
    );
}

/// Cells of the grate row's own band: the wall column, `grate_height` (2)
/// courses, at `z`.
fn grate_row_cells(
    region: Box3,
    wall_x: i32,
    y0: i32,
    height: i32,
) -> impl Iterator<Item = [i32; 3]> {
    (region.origin[2]..region.maximum()[2])
        .flat_map(move |z| (y0..y0 + height).map(move |y| [wall_x, y, z]))
}

/// The X-specific counterpart of `the_worn_tread_variant_is_not_counted_as_an_accent`:
/// the broken cell is the same material family as the intact grate, at a
/// different distress level, and must not read as an accent — proved on a
/// row short enough that an ungrouped reading of the same cells genuinely
/// would.
#[test]
fn the_broken_grate_variant_is_not_counted_as_an_accent() {
    let out = expand_at(&broken_grate(), GRATE_PALETTE_REGION, 1);
    let wall_x = GRATE_PALETTE_REGION.size[0] as i32 - 1;
    let cells: Vec<[i32; 3]> = grate_row_cells(GRATE_PALETTE_REGION, wall_x, 0, 2).collect();

    let grouped = family_shares(
        &out.model,
        cells.iter().copied(),
        &[("bars", &[GRATE_BLOCK, BROKEN_BLOCK])],
    );
    assert!(
        accent_overruns(&grouped).is_empty(),
        "the family-grouped grate row was flagged as carrying an accent: {grouped:?}"
    );

    let naive = family_shares(&out.model, cells.iter().copied(), &[]);
    let broken_share = naive.get(BROKEN_BLOCK).copied().unwrap_or(0.0);
    assert!(
        broken_share > ACCENT_CEILING,
        "the fixture does not give the naive reading real teeth: the break is only \
         {broken_share:.3} of the row"
    );
    assert!(
        !accent_overruns(&naive).is_empty(),
        "an ungrouped reading of the same cells did not trip the accent ceiling"
    );
}

/// ...and the same mirror still fires on a genuine accent: restyle the break
/// to a wholly unrelated material and, over the same cells and family map, it
/// reads as an overrun.
#[test]
fn the_palette_mirror_still_fires_on_a_genuine_accent_in_the_grate_row() {
    let mut restyled = broken_grate();
    restyled
        .set_role(
            "grate_broken",
            delvewright_grammar::ir::Paint::Block(BlockState::simple("gold_block")),
        )
        .unwrap();
    let out = expand_at(&restyled, GRATE_PALETTE_REGION, 1);
    let wall_x = GRATE_PALETTE_REGION.size[0] as i32 - 1;
    let cells: Vec<[i32; 3]> = grate_row_cells(GRATE_PALETTE_REGION, wall_x, 0, 2).collect();

    let grouped = family_shares(
        &out.model,
        cells.iter().copied(),
        &[("bars", &[GRATE_BLOCK])],
    );
    let overruns = accent_overruns(&grouped);
    assert!(
        overruns.iter().any(|(k, _)| k == "minecraft:gold_block"),
        "a genuinely unrelated material was not caught as an accent overrun: {grouped:?}"
    );
}

// ---------------------------------------------------------------------------
// L — the drop shaft
// ---------------------------------------------------------------------------

#[test]
fn a_drop_shaft_expands_deterministically() {
    let program = drop_shaft();
    let a = expand_at(&program, SHAFT_REGION, SHAFT_SEED);
    let b = expand_at(&program, SHAFT_REGION, SHAFT_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
    assert!(a.anchors.contains_key("anchor/spill"));
    assert!(a.anchors.contains_key("anchor/landing"));
}

/// The gate the entry is named for, half one: granting a player's own
/// free-fall (`reachable_with_fall`), the brink genuinely reaches the landing
/// — the structure is not simply broken in two.
#[test]
fn the_spill_falls_to_the_landing() {
    let out = expand_at(&drop_shaft(), SHAFT_REGION, SHAFT_SEED);
    let model = &out.model;
    let spill = out.anchors["anchor/spill"].pos;
    let landing = out.anchors["anchor/landing"].pos;
    assert!(standable(model, spill), "{spill:?}");
    assert!(standable(model, landing), "{landing:?}");

    let cells = standable_cells(model);
    let from: BTreeSet<[i32; 3]> = [spill].into_iter().collect();
    let to: BTreeSet<[i32; 3]> = [landing].into_iter().collect();
    assert!(
        reachable_with_fall(model, &cells, &from, &to),
        "the brink cannot even fall to its own landing"
    );
}

/// Half two, and the one the entry exists for: under the *plain* ±1 step walk
/// — no fall, since a fall never points up — the landing cannot reach the
/// brink. Proving the negative under `cliff_path`'s own stricter model is the
/// honest claim; the fall-capable model is only ever used going forward.
#[test]
fn the_landing_has_no_way_back_to_the_spill() {
    let out = expand_at(&drop_shaft(), SHAFT_REGION, SHAFT_SEED);
    let model = &out.model;
    let spill = out.anchors["anchor/spill"].pos;
    let landing = out.anchors["anchor/landing"].pos;
    let cells = standable_cells(model);
    let from: BTreeSet<[i32; 3]> = [landing].into_iter().collect();
    let to: BTreeSet<[i32; 3]> = [spill].into_iter().collect();
    assert!(
        !connected(&cells, &from, &to),
        "the landing walks straight back up to the brink — the shaft is not one-way"
    );
}

/// ...and the gate has teeth. `rescue_ladder` notches the entry floor at one
/// column — paired with a `drop` of 2, one notch is exactly enough to bridge
/// the gap (see the module note) — and the same plain-walk check must now find
/// a way back. Both programs are identical but for the knob, isolating it as
/// the one variable that flips the gate.
#[test]
fn a_rescued_shaft_has_a_way_back() {
    let mut sealed = drop_shaft();
    sealed.set_param("drop", 2).unwrap();
    sealed.set_param("head", 2).unwrap();
    let mut rescued = sealed.clone();
    rescued.set_param("rescue_ladder", 1).unwrap();

    let sealed_out = expand_at(&sealed, SHAFT_TEETH_REGION, SHAFT_SEED);
    let sealed_cells = standable_cells(&sealed_out.model);
    let s_spill: BTreeSet<[i32; 3]> = [sealed_out.anchors["anchor/spill"].pos]
        .into_iter()
        .collect();
    let s_landing: BTreeSet<[i32; 3]> = [sealed_out.anchors["anchor/landing"].pos]
        .into_iter()
        .collect();
    assert!(
        !connected(&sealed_cells, &s_landing, &s_spill),
        "the un-rescued shaft already has a way back — the fixture proves nothing"
    );

    let rescued_out = expand_at(&rescued, SHAFT_TEETH_REGION, SHAFT_SEED);
    let rescued_cells = standable_cells(&rescued_out.model);
    let r_spill: BTreeSet<[i32; 3]> = [rescued_out.anchors["anchor/spill"].pos]
        .into_iter()
        .collect();
    let r_landing: BTreeSet<[i32; 3]> = [rescued_out.anchors["anchor/landing"].pos]
        .into_iter()
        .collect();
    assert!(
        connected(&rescued_cells, &r_landing, &r_spill),
        "rescue_ladder notched the entry floor and the plain walk still reports no way back \
         — the gate proves nothing"
    );
}

// ---------------------------------------------------------------------------
// L — the dumbwaiter
// ---------------------------------------------------------------------------

#[test]
fn a_dumbwaiter_expands_deterministically() {
    let program = dumbwaiter();
    let a = expand_at(&program, DUCT_REGION, DUCT_SEED);
    let b = expand_at(&program, DUCT_REGION, DUCT_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
    assert!(a.anchors.contains_key("anchor/hatch"));
    assert!(a.anchors.contains_key("anchor/landing"));
}

#[test]
fn the_hatch_falls_to_the_landing() {
    let out = expand_at(&dumbwaiter(), DUCT_REGION, DUCT_SEED);
    let model = &out.model;
    let hatch = out.anchors["anchor/hatch"].pos;
    let landing = out.anchors["anchor/landing"].pos;
    assert!(standable(model, hatch), "{hatch:?}");
    assert!(standable(model, landing), "{landing:?}");

    let cells = standable_cells(model);
    let from: BTreeSet<[i32; 3]> = [hatch].into_iter().collect();
    let to: BTreeSet<[i32; 3]> = [landing].into_iter().collect();
    assert!(
        reachable_with_fall(model, &cells, &from, &to),
        "the hatch cannot even fall to its own landing"
    );
}

#[test]
fn the_landing_has_no_way_back_to_the_hatch() {
    let out = expand_at(&dumbwaiter(), DUCT_REGION, DUCT_SEED);
    let model = &out.model;
    let hatch = out.anchors["anchor/hatch"].pos;
    let landing = out.anchors["anchor/landing"].pos;
    let cells = standable_cells(model);
    let from: BTreeSet<[i32; 3]> = [landing].into_iter().collect();
    let to: BTreeSet<[i32; 3]> = [hatch].into_iter().collect();
    assert!(
        !connected(&cells, &from, &to),
        "the landing walks straight back up to the hatch — the duct is not one-way"
    );
}

/// The same knob, the same short-drop pairing `drop_shaft` uses.
#[test]
fn a_rescued_dumbwaiter_has_a_way_back() {
    let mut sealed = dumbwaiter();
    sealed.set_param("drop", 2).unwrap();
    sealed.set_param("head", 2).unwrap();
    let mut rescued = sealed.clone();
    rescued.set_param("rescue_ladder", 1).unwrap();

    let sealed_out = expand_at(&sealed, DUCT_TEETH_REGION, DUCT_SEED);
    let sealed_cells = standable_cells(&sealed_out.model);
    let s_hatch: BTreeSet<[i32; 3]> = [sealed_out.anchors["anchor/hatch"].pos]
        .into_iter()
        .collect();
    let s_landing: BTreeSet<[i32; 3]> = [sealed_out.anchors["anchor/landing"].pos]
        .into_iter()
        .collect();
    assert!(
        !connected(&sealed_cells, &s_landing, &s_hatch),
        "the un-rescued duct already has a way back — the fixture proves nothing"
    );

    let rescued_out = expand_at(&rescued, DUCT_TEETH_REGION, DUCT_SEED);
    let rescued_cells = standable_cells(&rescued_out.model);
    let r_hatch: BTreeSet<[i32; 3]> = [rescued_out.anchors["anchor/hatch"].pos]
        .into_iter()
        .collect();
    let r_landing: BTreeSet<[i32; 3]> = [rescued_out.anchors["anchor/landing"].pos]
        .into_iter()
        .collect();
    assert!(
        connected(&rescued_cells, &r_landing, &r_hatch),
        "rescue_ladder notched the entry floor and the plain walk still reports no way back \
         — the gate proves nothing"
    );
}

// ---------------------------------------------------------------------------
// The way up — the stair flight
// ---------------------------------------------------------------------------
//
// Every gate here is read with the **same** `standable` predicate and the same
// plain ±1-step `connected` walk the L family's one-wayness is proved with, and
// that is the whole design of this section: `drop_shaft` asserts
// `!connected(landing, spill)` and this asserts `connected(foot, head)`. One
// model of "can a body get there", two rules, opposite verdicts — rather than
// two independent ideas of reachability that could drift apart and both be
// green.

/// Anchor cells of a stem, as a one-element set for `connected`.
fn cell(out: &Expansion, name: &str) -> BTreeSet<[i32; 3]> {
    [out.anchors[name].pos].into_iter().collect()
}

#[test]
fn a_stair_flight_expands_deterministically() {
    let program = stair_flight();
    let a = expand_at(&program, FLIGHT_REGION, FLIGHT_SEED);
    let b = expand_at(&program, FLIGHT_REGION, FLIGHT_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
    assert!(a.anchors.contains_key("anchor/stair-foot"));
    assert!(a.anchors.contains_key("anchor/stair-head"));
    assert_eq!(
        indexed(&a.anchors, "stair-step").len(),
        8,
        "the fixture's eight treads are the binding count every gate below reads"
    );
}

/// **The gate the entry exists for.** A body walks from the foot landing to the
/// head landing under the plain ±1 step — no fall edge, because a fall never
/// points up — and walks back down again.
///
/// Both directions are asserted and they are deliberately the *same* claim:
/// `connected`'s edge relation is symmetric (every `dy` step has a matching
/// `-dy` step), so a graph that carries a climb carries the descent. Writing
/// both down is what makes the calibration against `drop_shaft` legible —
/// see `the_flight_and_the_drop_shaft_are_one_gate_read_twice`.
///
/// Binding: 66 standable cells, 2 landings, 8 treads.
#[test]
fn the_flight_walks_up_and_back_down() {
    let out = expand_at(&stair_flight(), FLIGHT_REGION, FLIGHT_SEED);
    let model = &out.model;
    let foot = out.anchors["anchor/stair-foot"].pos;
    let head = out.anchors["anchor/stair-head"].pos;
    assert!(standable(model, foot), "{foot:?}");
    assert!(standable(model, head), "{head:?}");

    let cells = standable_cells(model);
    assert_eq!(
        cells.len(),
        66,
        "the fixture's standable cell count moved; every gate below is read off it"
    );
    let from = cell(&out, "anchor/stair-foot");
    let to = cell(&out, "anchor/stair-head");
    assert!(
        connected(&cells, &from, &to),
        "the foot landing cannot walk up to the head landing — nothing in the \
         vocabulary ascends after all"
    );
    assert!(
        connected(&cells, &to, &from),
        "the head landing cannot walk back down"
    );
}

/// The calibration, written as one test so nobody has to take it on trust:
/// the same predicate, the same walk, the same direction of travel, run over
/// this rule and over `drop_shaft`, must disagree. A change that broke
/// `connected` into always-true would red the shaft's half; one that broke it
/// into always-false would red the flight's.
#[test]
fn the_flight_and_the_drop_shaft_are_one_gate_read_twice() {
    let flight = expand_at(&stair_flight(), FLIGHT_REGION, FLIGHT_SEED);
    let flight_cells = standable_cells(&flight.model);
    let climbs = connected(
        &flight_cells,
        &cell(&flight, "anchor/stair-foot"),
        &cell(&flight, "anchor/stair-head"),
    );

    let shaft = expand_at(&drop_shaft(), SHAFT_REGION, SHAFT_SEED);
    let shaft_cells = standable_cells(&shaft.model);
    let climbs_back = connected(
        &shaft_cells,
        &cell(&shaft, "anchor/landing"),
        &cell(&shaft, "anchor/spill"),
    );

    assert!(
        climbs && !climbs_back,
        "the two halves of the vertical family agree ({climbs} / {climbs_back}); \
         one predicate cannot be both the ascent gate and the one-way gate if it \
         answers the same for a stair and a drop"
    );
}

/// **A walk gate alone is vacuous**: a flat corridor walks perfectly in both
/// directions. So the rise is measured off the declared anchors and pinned, and
/// the control is a rule that is flat *by construction* — `boulder_stair`,
/// whose own module explains at length why it does not climb — read in the same
/// box by the same code.
///
/// Binding: 2 landings and 8 treads for the flight, 1 run anchor for the flat
/// control.
#[test]
fn the_head_landing_is_really_above_the_foot_and_a_flat_lane_is_not() {
    let out = expand_at(&stair_flight(), FLIGHT_REGION, FLIGHT_SEED);
    let foot = out.anchors["anchor/stair-foot"].pos;
    let head = out.anchors["anchor/stair-head"].pos;
    assert_eq!(
        head[1] - foot[1],
        7,
        "the flight's rise moved: foot {foot:?}, head {head:?}"
    );
    let steps = indexed(&out.anchors, "stair-step");
    assert_eq!(steps.len(), 8);

    // The control: the flat rule in the same box. Its own floor anchor and the
    // lowest cell of its lane are the same height, which is what "flat" means
    // and what this gate would report as a rise of zero.
    let flat = expand_at(&boulder_stair(), FLIGHT_REGION, FLIGHT_SEED);
    let lane: BTreeSet<i32> = standable_cells(&flat.model)
        .into_iter()
        .map(|c| c[1])
        .collect();
    assert_eq!(
        lane.len(),
        1,
        "boulder_stair is supposed to be flat; if its lane spans {} heights the \
         control has stopped controlling anything",
        lane.len()
    );
}

/// **Every riser is exactly one block, and every tread is ground.** The treads
/// are read off the declared anchors in index order — which, as everywhere in
/// this vocabulary, runs *against* travel: `stair-step-1` is the topmost tread,
/// so `Y` decreases by one and `Z` increases by one `tread` as the index rises.
///
/// Binding: 8 treads, 7 consecutive pairs.
#[test]
fn every_riser_is_one_block_and_every_tread_is_standable() {
    let out = expand_at(&stair_flight(), FLIGHT_REGION, FLIGHT_SEED);
    let model = &out.model;
    let steps = indexed(&out.anchors, "stair-step");
    assert_eq!(steps.len(), 8);
    for step in &steps {
        assert!(standable(model, *step), "tread {step:?} is not ground");
    }
    let mut pairs = 0;
    for w in steps.windows(2) {
        let (hi, lo) = (w[0], w[1]);
        assert_eq!(hi[1] - lo[1], 1, "riser between {lo:?} and {hi:?}");
        assert_eq!(lo[2] - hi[2], 2, "run between {lo:?} and {hi:?}");
        pairs += 1;
    }
    assert_eq!(pairs, 7, "seven risers between eight treads");

    // The lowest tread is level with the foot landing and the highest with the
    // head landing: a flight arrives on its landings, it does not step up onto
    // them.
    assert_eq!(steps[7][1], out.anchors["anchor/stair-foot"].pos[1]);
    assert_eq!(steps[0][1], out.anchors["anchor/stair-head"].pos[1]);
}

/// **The teeth, in the direction that actually drifts.** `broken_step` raises
/// one tread — the last one, picked out of the recursion by a guard on the
/// remaining run rather than by an index — so exactly one riser becomes 2 and
/// the next becomes 0.
///
/// This is what a stair fails as: not demolished, not severed, just one step a
/// body cannot make. Everything below the break still walks, the foot landing
/// is still ground, the model still has all 8 treads and all its walls — and
/// the head landing is stranded. A gate that only proved the shaft was not
/// broken in two would be green on this.
#[test]
fn a_single_raised_tread_strands_the_head_landing() {
    let sound = stair_flight();
    let mut broken = sound.clone();
    broken.set_param("broken_step", 1).unwrap();

    let sound_out = expand_at(&sound, FLIGHT_REGION, FLIGHT_SEED);
    let sound_cells = standable_cells(&sound_out.model);
    assert!(
        connected(
            &sound_cells,
            &cell(&sound_out, "anchor/stair-foot"),
            &cell(&sound_out, "anchor/stair-head"),
        ),
        "the sound flight already does not climb — the fixture proves nothing"
    );

    let out = expand_at(&broken, FLIGHT_REGION, FLIGHT_SEED);
    let cells = standable_cells(&out.model);
    let foot = cell(&out, "anchor/stair-foot");
    let head = cell(&out, "anchor/stair-head");
    assert!(
        !connected(&cells, &foot, &head),
        "a tread was raised a whole block and the climb still gets through — \
         the both-ways gate cannot see a step nobody can make"
    );
    assert!(
        !connected(&cells, &head, &foot),
        "the descent is unaffected, so the break is not on the route"
    );

    // ...and what was caught is a step, not a demolition. Exactly one riser is
    // wrong, both landings are still ground, and the flight still walks from
    // the foot up to the tread below the break.
    let steps = indexed(&out.anchors, "stair-step");
    assert_eq!(
        steps.len(),
        8,
        "the break removed a tread instead of raising it"
    );
    let bad: Vec<i32> = steps.windows(2).map(|w| w[0][1] - w[1][1]).collect();
    assert_eq!(
        bad,
        vec![2, 1, 1, 1, 1, 1, 1],
        "exactly one riser should be two blocks and the rest one"
    );
    assert!(standable(&out.model, out.anchors["anchor/stair-foot"].pos));
    assert!(standable(&out.model, out.anchors["anchor/stair-head"].pos));
    assert!(
        connected(&cells, &foot, &[steps[1]].into_iter().collect()),
        "the flight below the break stopped walking too — that is a demolished \
         shaft, not a broken step"
    );
}

/// **It is a shaft: both long faces are solid, every cell of them.** Read off
/// the model rather than off the rule, and its teeth are permanent rather than
/// a knob — `tee_passage` is the same wall-with-one-opening construction and
/// the same reading must find its doorway. One reading, two rules, opposite
/// answers.
///
/// Binding: 616 wall cells (2 faces × 14 × 22) for the flight, 2 open cells for
/// the control.
#[test]
fn the_shaft_is_walled_on_both_long_faces() {
    let out = expand_at(&stair_flight(), FLIGHT_REGION, FLIGHT_SEED);
    let (walled, open) = side_face_cells(&out.model);
    assert_eq!(walled + open, 616, "the fixture's two side faces");
    assert_eq!(
        open, 0,
        "the shaft has {open} open cells in a side face — a body can walk out of \
         it, so it is not a shaft"
    );

    // The control, in the same box: a rule that deliberately opens one side
    // face. If this reads zero too, the reading above is measuring nothing.
    let tee = expand_at(&tee_passage(), FLIGHT_REGION, FLIGHT_SEED);
    let (_, tee_open) = side_face_cells(&tee.model);
    assert_eq!(
        tee_open, 2,
        "tee_passage's doorway did not show up in the same reading"
    );
}

/// Solid and open cell counts over the model's two extreme `X` planes — the
/// faces every rule in this vocabulary walls (`docs/reference/grammar.md` §5c).
fn side_face_cells(model: &VoxelModel) -> (usize, usize) {
    let region = model.region();
    let (x_min, x_max) = (region.origin[0], region.maximum()[0] - 1);
    let mut walled = 0;
    let mut open = 0;
    for pos in region.positions() {
        if pos[0] != x_min && pos[0] != x_max {
            continue;
        }
        if solid(model, pos) {
            walled += 1;
        } else {
            open += 1;
        }
    }
    (walled, open)
}

/// **The run is the only way up.** The same cut `cliff_path`, `ambush_door` and
/// `boulder_stair` use on their own lanes: delete one `Z` slice of standable
/// cells mid-run and the two landings must part company — under the *fall*
/// model as well as the plain walk, since a fall edge only ever adds routes and
/// a stairwell is open above its own run.
///
/// Binding: 66 standable cells, 6 of them in the cut slice.
#[test]
fn the_run_is_the_only_way_between_the_landings() {
    let out = expand_at(&stair_flight(), FLIGHT_REGION, FLIGHT_SEED);
    let model = &out.model;
    let cells = standable_cells(model);
    let foot = cell(&out, "anchor/stair-foot");
    let head = cell(&out, "anchor/stair-head");
    assert!(reachable_with_fall(model, &cells, &foot, &head));

    // The middle tread's own Z columns, whichever cells they turned out to be.
    let steps = indexed(&out.anchors, "stair-step");
    let cut_z = steps[4][2];
    let cut: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[2] != cut_z && c[2] != cut_z + 1)
        .collect();
    assert_eq!(
        cells.len() - cut.len(),
        6,
        "the cut removed the wrong number of cells"
    );
    assert!(
        !reachable_with_fall(model, &cut, &foot, &head),
        "the landings are still joined with a slice of the run deleted, so the \
         run is not the route"
    );
}

/// A box that cannot hold `MIN_STEPS` treads is a refusal naming the rule, not
/// a two-step doorstep that would pass every gate above.
///
/// Binding: 1 refusal, against the same box one cell longer, which builds.
#[test]
fn a_box_too_short_to_climb_is_refused_not_flattened() {
    let program = stair_flight();
    let too_short = Box3::at_origin([5, 14, 11]);
    let err = expand(&program, too_short, &ExpandOptions::seeded(FLIGHT_SEED)).unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("flight_plan"),
        "the refusal does not name the rule that refused: {text}"
    );
    let just_long_enough = Box3::at_origin([5, 14, 12]);
    let out = expand_at(&program, just_long_enough, FLIGHT_SEED);
    assert_eq!(
        indexed(&out.anchors, "stair-step").len(),
        stair_flight::MIN_STEPS as usize,
        "the smallest legal flight should lay exactly MIN_STEPS treads"
    );
}

// ---------------------------------------------------------------------------
// F — the far-side bar
// ---------------------------------------------------------------------------

#[test]
fn a_far_side_bar_expands_deterministically() {
    let program = far_side_bar();
    let a = expand_at(&program, BAR_REGION, BAR_SEED);
    let b = expand_at(&program, BAR_REGION, BAR_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
}

/// The gate this entry exists for: while the bar stands, the near side simply
/// cannot reach `anchor/unlock` — the two rooms are not connected at all, not
/// merely "the shortest route is longer".
#[test]
fn the_near_side_cannot_reach_the_unlock_while_barred() {
    let out = expand_at(&far_side_bar(), BAR_REGION, BAR_SEED);
    let model = &out.model;
    let unlock = out.anchors["anchor/unlock"].pos;
    let gate = out.anchors["anchor/gate"].pos;
    assert!(standable(model, unlock), "{unlock:?}");
    assert!(
        gate[2] > unlock[2],
        "the gate {gate:?} must sit between the near side and the far-side unlock {unlock:?}"
    );

    let cells = standable_cells(model);
    let near: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] > gate[2]).collect();
    let far: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] < gate[2]).collect();
    assert!(!near.is_empty() && !far.is_empty());
    assert!(
        !connected(&cells, &near, &far),
        "the near side reaches the far side while the bar stands — the shortcut has no far side \
         to earn"
    );
    let unlock_set: BTreeSet<[i32; 3]> = [unlock].into_iter().collect();
    assert!(
        connected(&cells, &far, &unlock_set),
        "the unlock is not even reachable from its own room"
    );
}

/// ...and the gate has teeth. `unbarred = 1` swaps the fill for air and
/// nothing else changes; the same check must now find the two rooms connected
/// through exactly that doorway — proof both that the wall has no other gap,
/// and that the bar (not some second opening) was what sealed it.
#[test]
fn unbarring_the_door_connects_the_rooms() {
    let mut open = far_side_bar();
    open.set_param("unbarred", 1).unwrap();
    let out = expand_at(&open, BAR_REGION, BAR_SEED);
    let model = &out.model;
    let gate = out.anchors["anchor/gate"].pos;
    let cells = standable_cells(model);
    let near: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] > gate[2]).collect();
    let far: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[2] < gate[2]).collect();
    assert!(
        connected(&cells, &near, &far),
        "unbarring the door and the two rooms still do not connect — the gate proves nothing"
    );
}

// ---------------------------------------------------------------------------
// J — the tee passage
// ---------------------------------------------------------------------------

/// Every open cell of the passage's two side-face planes: local `X`-min (which
/// carries the doorway) and local `X`-max (which must not carry anything).
///
/// Read off the *model*, not off `door_height` — the claim is about how many
/// holes the rule actually cut, and recomputing it from the parameter would
/// prove only that the arithmetic in the test matches the arithmetic in the
/// rule.
fn side_face_openings(out: &Expansion) -> (Vec<[i32; 3]>, Vec<[i32; 3]>) {
    let region = out.model.region();
    let min_x = region.origin[0];
    let max_x = min_x + region.size[0] as i32 - 1;
    let open = |x: i32| -> Vec<[i32; 3]> {
        region
            .positions()
            .filter(|p| p[0] == x && !solid(&out.model, *p))
            .collect()
    };
    (open(min_x), open(max_x))
}

/// The standable cells at the two ends of the lane's travel axis: local `Z`-max,
/// where the player walks in, and `Z`-min, where they leave.
fn lane_ends(model: &VoxelModel) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let region = model.region();
    let far = region.origin[2] + region.size[2] as i32 - 1;
    let cells = standable_cells(model);
    (
        cells.iter().copied().filter(|c| c[2] == far).collect(),
        cells
            .iter()
            .copied()
            .filter(|c| c[2] == region.origin[2])
            .collect(),
    )
}

#[test]
fn a_tee_passage_expands_deterministically() {
    let program = tee_passage();
    let a = expand_at(&program, TEE_REGION, TEE_SEED);
    let b = expand_at(&program, TEE_REGION, TEE_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
}

/// Gate 1 and gate 2 together, because they are the two halves of one claim: the
/// piece is still a **chain segment** (standable end to end, so a zone can lay
/// it in a piece run) and it has exactly **one** side opening (so the branch a
/// zone parks beside it is the only thing off the route).
///
/// Binding: 120 side-wall cells examined (2 faces × 5 tall × 12 long), of which
/// exactly 2 are open — the `door_height` cells of the doorway, all in the
/// `X`-min face, all at one `Z`.
#[test]
fn the_tee_passage_is_a_lane_with_one_side_opening() {
    let out = expand_at(&tee_passage(), TEE_REGION, TEE_SEED);
    let cells = standable_cells(&out.model);
    let door = out.anchors["anchor/branch-door"].pos;
    assert!(standable(&out.model, door), "the doorway cell {door:?}");
    assert_eq!(
        out.anchors["anchor/branch-door"].facing.as_str(),
        "west",
        "the branch door must look across travel, at the box the branch occupies"
    );

    let (entry, exit) = lane_ends(&out.model);
    assert!(!entry.is_empty() && !exit.is_empty(), "the lane has ends");
    assert!(
        connected(&cells, &entry, &exit),
        "the tee does not run end to end, so it cannot be a segment of a chain"
    );

    let (near_face, far_face) = side_face_openings(&out);
    assert!(
        far_face.is_empty(),
        "the far side face is open at {far_face:?} — a zone would have a branch it \
         never declared, on the side it never proved"
    );
    assert_eq!(
        near_face.len(),
        tee_passage().params["door_height"] as usize,
        "the doorway is {} cells, not `door_height`: {near_face:?}",
        near_face.len()
    );
    let door_zs: BTreeSet<i32> = near_face.iter().map(|c| c[2]).collect();
    assert_eq!(
        door_zs,
        [door[2]].into_iter().collect::<BTreeSet<i32>>(),
        "the openings in the near face are not one doorway at the anchor's own Z"
    );
}

/// Gate 3, and the whole difference between this rule and `far_side_bar`: the
/// doorway is **beside** the route. Delete its column and the lane still walks
/// end to end.
///
/// The teeth are permanent rather than a knob, because the defect this gate
/// exists to catch is not a mis-set parameter — it is *building the other rule*.
/// So the same cut is run against an unbarred `far_side_bar` in the same box:
/// one construction, one cut, opposite answers. A cut that could not sever
/// anything would pass this gate vacuously.
///
/// Binding: 37 standable cells re-walked without the doorway's column, against
/// the bar's own cells re-walked without its opening.
#[test]
fn the_tee_passages_doorway_is_beside_the_route_not_on_it() {
    let out = expand_at(&tee_passage(), TEE_REGION, TEE_SEED);
    let cells = standable_cells(&out.model);
    assert_eq!(cells.len(), 37, "the lane's standable cells");
    let door = out.anchors["anchor/branch-door"].pos;
    let (entry, exit) = lane_ends(&out.model);

    let cut: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[0] != door[0] || c[2] != door[2])
        .collect();
    assert_eq!(
        cut.len(),
        cells.len() - 1,
        "exactly the doorway was removed"
    );
    assert!(
        connected(&cut, &entry, &exit),
        "plugging the branch doorway severed the lane — the tee put its opening on \
         the route, which is a `far_side_bar` and not a junction"
    );

    // The red side of the same cut: the rule this one is a rotation of. Its
    // opening *is* the route, so plugging it severs the box end to end.
    let mut open = far_side_bar();
    open.set_param("unbarred", 1).unwrap();
    let bar = expand_at(&open, TEE_REGION, TEE_SEED);
    let bar_cells = standable_cells(&bar.model);
    let gate = bar.anchors["anchor/gate"].pos;
    let (bar_entry, bar_exit) = lane_ends(&bar.model);
    assert!(
        connected(&bar_cells, &bar_entry, &bar_exit),
        "the unbarred fixture is not even a route, so the cut below proves nothing"
    );
    let bar_cut: BTreeSet<[i32; 3]> = bar_cells
        .iter()
        .copied()
        .filter(|c| c[2] != gate[2])
        .collect();
    assert!(
        !connected(&bar_cut, &bar_entry, &bar_exit),
        "plugging the bar's own opening left the box connected — this cut cannot \
         sever anything, so the tee passing it means nothing"
    );
}

/// ...and the teeth. `sealed = 1` fills the doorway with the shell material and
/// changes nothing else: the one-opening count must drop to zero while the lane
/// walks exactly as it did, so what the gates above measured is the doorway and
/// not the shape of the box.
#[test]
fn sealing_the_tee_passage_closes_its_only_opening() {
    let mut sealed = tee_passage();
    sealed.set_param("sealed", 1).unwrap();
    let out = expand_at(&sealed, TEE_REGION, TEE_SEED);
    let (near_face, far_face) = side_face_openings(&out);
    assert!(
        near_face.is_empty() && far_face.is_empty(),
        "the doorway was filled and the side faces still read as open: {near_face:?} \
         / {far_face:?} — the gate proves nothing"
    );

    // The control: a filled doorway, not a filled passage. The lane is still the
    // same lane, minus the one cell that was the doorway.
    let cells = standable_cells(&out.model);
    let (entry, exit) = lane_ends(&out.model);
    assert!(connected(&cells, &entry, &exit));
    assert_eq!(
        cells.len(),
        standable_cells(&expand_at(&tee_passage(), TEE_REGION, TEE_SEED).model).len() - 1,
        "sealing the doorway moved more than the doorway"
    );
}

// ---------------------------------------------------------------------------
// T — the causeway
// ---------------------------------------------------------------------------

/// Where the flooded ward begins: the lowest `Z` any water stands in.
/// Everything short of it is the guard station's own footprint.
///
/// Derived from the model rather than from `guard_len`, because "the causeway"
/// means the part of the spline that crosses the flood, and the flood is what
/// says where that is.
fn ward_start(model: &VoxelModel) -> i32 {
    model
        .region()
        .positions()
        .filter(|&p| model.get(p).is_some_and(|b| b.name == "minecraft:water"))
        .map(|p| p[2])
        .min()
        .expect("the ward built no water at all")
}

/// The whole spline at berm height: the raised lane's `X` column and foot
/// height, along the entire piece. Filtered on `Y` as well as `X` — the guard
/// station's post shares the spline's `X` by construction, and without the `Y`
/// filter the guard's own standable cell would read as part of the lane.
fn spine_cells(model: &VoxelModel, causeway_head: [i32; 3]) -> BTreeSet<[i32; 3]> {
    standable_cells(model)
        .into_iter()
        .filter(|c| c[0] == causeway_head[0] && c[1] == causeway_head[1])
        .collect()
}

/// The causeway proper: the spline **over the flooded ward**.
///
/// With `berm_gate` shut this is the whole spline, because the plinth carries
/// no standable cell. With it open the spline additionally runs under the
/// guard station, and those cells are the *gatehouse's* passage rather than the
/// crossing — see [`the_berm_gate_opens_a_lane_past_the_post`], which counts
/// them and states what the post can and cannot see of them, instead of leaving
/// them quietly inside a gate that is about the crossing.
fn causeway_cells(model: &VoxelModel, causeway_head: [i32; 3]) -> BTreeSet<[i32; 3]> {
    let ward = ward_start(model);
    spine_cells(model, causeway_head)
        .into_iter()
        .filter(|c| c[2] >= ward)
        .collect()
}

/// The causeway's far end: its own lowest `Z`, which is the boundary against
/// the guard station's cantilever — not world `Z = 0`, which is inside the
/// guard station's own (separately structured) footprint.
fn causeway_far_end(lane: &BTreeSet<[i32; 3]>) -> BTreeSet<[i32; 3]> {
    let far_z = lane.iter().map(|c| c[2]).min().expect("a non-empty lane");
    lane.iter().copied().filter(|c| c[2] == far_z).collect()
}

#[test]
fn a_causeway_expands_deterministically() {
    let program = causeway();
    let a = expand_at(&program, CAUSEWAY_REGION, CAUSEWAY_SEED);
    let b = expand_at(&program, CAUSEWAY_REGION, CAUSEWAY_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
}

/// The first gate: the causeway is standable end to end, and stepping off it
/// is not — every flood cell fails `standable` outright, because its foot
/// cell is water, not air.
#[test]
fn the_causeway_is_standable_and_the_flood_is_not() {
    let out = expand_at(&causeway(), CAUSEWAY_REGION, CAUSEWAY_SEED);
    let model = &out.model;
    let head = out.anchors["anchor/causeway-head"].pos;
    let elite = out.anchors["anchor/elite"].pos;
    assert!(standable(model, head), "{head:?}");

    let lane = causeway_cells(model, head);
    assert!(
        lane.len() >= 6,
        "the causeway fixture is too short to test: {lane:?}"
    );
    let far_end: BTreeSet<[i32; 3]> = causeway_far_end(&lane);
    let near_end: BTreeSet<[i32; 3]> = [head].into_iter().collect();
    assert!(
        connected(&lane, &near_end, &far_end),
        "the causeway does not connect its own two ends"
    );

    // Every flooded cell (both sides, over the whole ward) is water at foot
    // level, and water is not air, so it is never standable.
    let flooded: Vec<[i32; 3]> = CAUSEWAY_REGION
        .positions()
        .filter(|&p| model.get(p).is_some_and(|b| b.name == "minecraft:water"))
        .collect();
    assert!(!flooded.is_empty(), "the ward built no water at all");
    for cell in &flooded {
        let foot = [cell[0], cell[1] + 1, cell[2]];
        assert!(
            !standable(model, foot),
            "{foot:?}, directly over water {cell:?}, is standable — the flood is not doing its job"
        );
    }
    // The post shares the causeway's own `X` (a direct line down it is the
    // whole point) but stands in the guard station's own `Z` zone, short of
    // the ward the flood cells above were just swept over.
    assert_eq!(elite[0], head[0]);
    assert!(elite[2] < flooded.iter().map(|c| c[2]).min().unwrap());
}

/// The second gate: the guard station sees every standable causeway cell —
/// **at every width the rule accepts**, not at the one the fixture happens to
/// use.
///
/// The sweep is the gate, and it is the gate because the narrow form was
/// passing for the wrong reason. The post used to mark the centre of its own
/// full-width column while the berm sat at the centre of a five-way section;
/// those are the same cell only when `X` is odd, and both fixtures in the repo
/// were odd. At even widths the guard stood over the flood one cell off its own
/// causeway and went blind on 20 of the 22 lane cells — green gate, wrong
/// geometry. A rule whose claim is "the post commands the spine" has to make
/// that claim at every box it accepts.
///
/// Binding: 9 widths (5..=13, five odd and four even), 22 lane cells each,
/// 198 sightlines, plus 9 assertions that the post is over the berm at all.
#[test]
fn the_guard_station_sees_the_whole_causeway() {
    let mut widths = 0;
    let mut sightlines = 0;
    for x in 5u32..=13 {
        let region = Box3::at_origin([x, 14, 24]);
        let out = expand_at(&causeway(), region, CAUSEWAY_SEED);
        let model = &out.model;
        let elite = out.anchors["anchor/elite"].pos;
        let head = out.anchors["anchor/causeway-head"].pos;
        assert_eq!(
            elite[0], head[0],
            "at width {x} the post stands at X={} and the berm at X={} — the guard is not \
             over its own causeway",
            elite[0], head[0]
        );
        let lane = causeway_cells(model, head);
        assert!(lane.len() >= 6, "width {x}: {lane:?}");
        for cell in &lane {
            if let Err(blocker) = sees(model, elite, *cell) {
                panic!(
                    "at width {x} the post {elite:?} cannot see the causeway cell {cell:?}: \
                     {blocker:?} is in the way"
                );
            }
            sightlines += 1;
        }
        widths += 1;
    }
    assert_eq!(
        (widths, sightlines),
        (9, 198),
        "the sweep examined {widths} widths and {sightlines} sightlines"
    );
}

/// ...and the gate has teeth. `obstruct = 1` stands one solid cell in the
/// causeway's own line, one course above the two cells `standable` needs
/// clear — so the check that catches it is catching blindness, not a severed
/// crossing.
#[test]
fn a_pillar_in_the_causeway_blinds_the_post() {
    let mut blocked = causeway();
    blocked.set_param("obstruct", 1).unwrap();
    let out = expand_at(&blocked, CAUSEWAY_REGION, CAUSEWAY_SEED);
    let model = &out.model;
    let elite = out.anchors["anchor/elite"].pos;
    let head = out.anchors["anchor/causeway-head"].pos;
    let lane = causeway_cells(model, head);
    assert!(!lane.is_empty());

    let blind: Vec<[i32; 3]> = lane
        .iter()
        .copied()
        .filter(|cell| sees(model, elite, *cell).is_err())
        .collect();
    assert!(
        !blind.is_empty(),
        "a pillar was stood in the causeway's own line of sight and the post still saw every \
         cell — the gate proves nothing"
    );
    // The causeway is still walkable end to end: what was caught is
    // blindness, not impassability.
    let far_end: BTreeSet<[i32; 3]> = causeway_far_end(&lane);
    let near_end: BTreeSet<[i32; 3]> = [head].into_iter().collect();
    assert!(connected(&lane, &near_end, &far_end));
}

/// The causeway's two faces along travel: what is standable at local `Z`-max
/// (the approach the player arrives on) and at `Z`-min (the far side of the
/// guard post). A zone chains pieces by exactly these faces, so "can a route
/// pass through this piece" is "does one face reach the other".
fn causeway_faces(model: &VoxelModel) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let region = model.region();
    let cells = standable_cells(model);
    let near = region.origin[2];
    let far = near + region.size[2] as i32 - 1;
    (
        cells.iter().copied().filter(|c| c[2] == far).collect(),
        cells.iter().copied().filter(|c| c[2] == near).collect(),
    )
}

/// A box wide enough to show the lane is not a fixture accident, tall enough
/// for the default post, and long enough that `z(Largest)` keeps travel on
/// world `Z`.
const CAUSEWAY_GATE_REGION: Box3 = Box3::at_origin([9, 14, 24]);

/// The third gate: `berm_gate` carries the berm through the post, and changes
/// nothing else about the post.
///
/// Four claims, and the fourth is the one that makes the first three worth
/// having — a lane that opened the piece by demolishing the guard post would
/// satisfy "chainable" and destroy the rule.
///
/// 1. **With the gate open the piece is walkable through**: its `Z`-max face
///    reaches its `Z`-min face under [`connected`] — the same ±1-step walk over
///    standable cells every other gate in this file uses. That model is
///    *test-local* (see [`reachable_with_fall`]'s note) and it does **not**
///    model a horizontal jump; here that only ever under-counts routes, so a
///    green means the lane is walkable, never merely jumpable.
/// 2. **With the gate shut it is not** — the terminus the rule is by default,
///    which is also the teeth for claim 1: the two runs differ in one
///    parameter, and only in that parameter.
/// 3. **The post is still not a landing.** The guard's own floor is unreachable
///    from either face under the same walk *and* under the permissive
///    walk-and-fall model, so the lane did not turn the post into a mezzanine.
/// 4. **The post is still a post**: the sightline gate above re-run with the
///    lane open, every width, every lane cell.
///
/// Binding: 2 configurations of the fixture, 9 widths swept for the sightline
/// with the lane open, 198 sightlines, 1 exit-lane cell open vs 0 shut.
#[test]
fn the_berm_gate_opens_a_lane_past_the_post() {
    let mut open = causeway();
    open.set_param("berm_gate", 1).expect("the knob exists");
    let out = expand_at(&open, CAUSEWAY_GATE_REGION, CAUSEWAY_SEED);
    let model = &out.model;
    let head = out.anchors["anchor/causeway-head"].pos;
    let elite = out.anchors["anchor/elite"].pos;
    let cells = standable_cells(model);
    let (entry, exit) = causeway_faces(model);

    // 1. The lane runs through.
    assert!(
        connected(&cells, &entry, &exit),
        "the gate is open and the piece still does not cross itself"
    );
    // ...and the cell it comes out at is the berm's own column at berm height,
    // not some other way round: exactly one exit cell at the berm's `X` and `Y`.
    let exit_lane: BTreeSet<[i32; 3]> = exit
        .iter()
        .copied()
        .filter(|c| c[0] == head[0] && c[1] == head[1])
        .collect();
    assert_eq!(
        exit_lane.len(),
        1,
        "the far face carries {} cells of berm-height lane, not one: {exit_lane:?}",
        exit_lane.len()
    );

    // 2. The teeth: shut the gate, change nothing else.
    let shut = expand_at(&causeway(), CAUSEWAY_GATE_REGION, CAUSEWAY_SEED);
    let shut_cells = standable_cells(&shut.model);
    let (shut_entry, shut_exit) = causeway_faces(&shut.model);
    assert!(
        !connected(&shut_cells, &shut_entry, &shut_exit),
        "the gate is shut and the piece crosses anyway — claim 1 proves nothing"
    );
    assert!(
        !shut_exit.iter().any(|c| c[0] == head[0] && c[1] == head[1]),
        "the shut piece already had a berm-height cell on its far face"
    );

    // 3. Still not a landing: the guard's floor is off the walk from both ends
    // of the lane, under the plain step and under the permissive fall model.
    // The sources are the *lane*, not the faces: the guard's own cell is on the
    // far face (it is standable, at the far face's `Z`), so "the exit face
    // reaches the post" would be true by containment and prove nothing.
    let post: BTreeSet<[i32; 3]> = [elite].into_iter().collect();
    assert!(standable(model, elite), "the guard has no floor: {elite:?}");
    for (name, from) in [("entry", &entry), ("exit lane", &exit_lane)] {
        assert!(
            !connected(&cells, from, &post),
            "the {name} face walks up onto the guard post {elite:?} — it is a landing now"
        );
        assert!(
            !reachable_with_fall(model, &cells, from, &post),
            "the {name} face reaches the guard post {elite:?} even before jumping is considered"
        );
    }

    // 4. The post is still a post, at every width the sweep above covers, and
    // over the same 22 crossing cells per width that the shut piece offers.
    let mut sightlines = 0;
    for x in 5u32..=13 {
        let out = expand_at(&open, Box3::at_origin([x, 14, 24]), CAUSEWAY_SEED);
        let model = &out.model;
        let elite = out.anchors["anchor/elite"].pos;
        let head = out.anchors["anchor/causeway-head"].pos;
        assert_eq!(elite[0], head[0], "width {x}, lane open");
        let crossing = causeway_cells(model, head);
        assert_eq!(crossing.len(), 22, "width {x}, lane open");
        for cell in &crossing {
            if let Err(blocker) = sees(model, elite, *cell) {
                panic!(
                    "with the lane open at width {x} the post {elite:?} cannot see {cell:?}: \
                     {blocker:?} is in the way"
                );
            }
            sightlines += 1;
        }
    }
    assert_eq!(
        sightlines, 198,
        "the open-lane sweep saw {sightlines} cells"
    );

    // 5. ...and the cells the lane adds are named rather than absorbed. The
    // gatehouse's own passage runs under the guard's floor, so the guard cannot
    // see it — that is what "pass under" means, and it is stated and counted
    // here instead of being quietly inside gate 2's binding. It is bounded: the
    // whole of it is `guard_len` cells at the very end of the crossing, and
    // every one of them is under the post's own floor.
    let gatehouse: BTreeSet<[i32; 3]> = spine_cells(model, head)
        .difference(&causeway_cells(model, head))
        .copied()
        .collect();
    assert_eq!(
        gatehouse.len(),
        2,
        "the lane adds cells outside the guard station: {gatehouse:?}"
    );
    for cell in &gatehouse {
        assert!(
            cell[1] < elite[1],
            "a gatehouse cell {cell:?} is not below the guard's floor {elite:?}"
        );
        assert!(
            sees(model, elite, *cell).is_err(),
            "the guard can see {cell:?} under its own floor — then the lane is not a way past"
        );
    }
}

/// ...and a post too short to be tunnelled is refused, not built with a
/// crawlspace under it.
///
/// The lane needs the two cells `standable` wants clear plus the course of
/// stone that is the post's own floor — [`MIN_GATE_RISE`] in all. One less and
/// there is no applicable alternative, which is the same refusal every other
/// undersized causeway gets; one more and the piece walks through.
///
/// Binding: `tower_rise` at `MIN_GATE_RISE - 1` (refused, by rule name) and at
/// `MIN_GATE_RISE` (built, and crossed).
#[test]
fn a_post_too_short_to_tunnel_refuses_the_lane() {
    let mut low = causeway();
    low.set_param("berm_gate", 1).unwrap();
    low.set_param("tower_rise", MIN_GATE_RISE - 1).unwrap();
    let err = expand(
        &low,
        CAUSEWAY_GATE_REGION,
        &ExpandOptions::seeded(CAUSEWAY_SEED),
    )
    .expect_err("a post that cannot carry a lane must refuse the lane");
    let text = err.to_string();
    assert!(
        text.contains("ward_alts") && text.contains("no alternative"),
        "the refusal does not name the rule that refused: {text}"
    );
    // ...and the same knob one notch higher is a piece that crosses, so what
    // was refused is the geometry and not the knob.
    let mut just_enough = causeway();
    just_enough.set_param("berm_gate", 1).unwrap();
    just_enough.set_param("tower_rise", MIN_GATE_RISE).unwrap();
    let out = expand_at(&just_enough, CAUSEWAY_GATE_REGION, CAUSEWAY_SEED);
    let cells = standable_cells(&out.model);
    let (entry, exit) = causeway_faces(&out.model);
    assert!(
        connected(&cells, &entry, &exit),
        "the shortest post the rule will tunnel does not actually carry a lane"
    );
}

// ---------------------------------------------------------------------------
// E — elite ground
// ---------------------------------------------------------------------------

/// The two flank bands, each independently walked end to end. Returns how
/// many of {west, east} genuinely connect the approach to the exit — the
/// "assert the count" the entry asks for, not an eyeballed layout.
fn flank_route_count(model: &VoxelModel, elite: [i32; 3], far_z: i32) -> usize {
    let cells = standable_cells(model);
    let west: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[0] < elite[0] - MIN_RADIUS as i32)
        .collect();
    let east: BTreeSet<[i32; 3]> = cells
        .iter()
        .copied()
        .filter(|c| c[0] > elite[0] + MIN_RADIUS as i32)
        .collect();
    [west, east]
        .into_iter()
        .filter(|band| {
            let entry: BTreeSet<[i32; 3]> =
                band.iter().copied().filter(|c| c[2] == far_z).collect();
            let exit: BTreeSet<[i32; 3]> = band.iter().copied().filter(|c| c[2] == 0).collect();
            !entry.is_empty() && !exit.is_empty() && connected(band, &entry, &exit)
        })
        .count()
}

#[test]
fn elite_ground_expands_deterministically() {
    let program = elite_ground();
    let a = expand_at(&program, ARENA_REGION, ARENA_SEED);
    let b = expand_at(&program, ARENA_REGION, ARENA_SEED);
    assert_eq!(a.model.canonical_bytes(), b.model.canonical_bytes());
    assert_eq!(a.anchors, b.anchors);
}

/// The first gate: the engagement circle — Chebyshev distance `radius` from
/// `anchor/elite` — is open ground, at least 9×9 at the entry's enforced
/// minimum. No fog-gate motif: there is no wall anywhere at the circle's own
/// boundary, which `the_open_floor_has_two_flank_routes` below is the
/// positive proof of (a fog-gate-shaped wall is exactly what that gate's
/// teeth test adds, and exactly what it catches).
#[test]
fn the_engagement_circle_is_open_ground() {
    let out = expand_at(&elite_ground(), ARENA_REGION, ARENA_SEED);
    let model = &out.model;
    let elite = out.anchors["anchor/elite"].pos;
    assert!(standable(model, elite), "{elite:?}");

    let diam = 2 * MIN_RADIUS + 1;
    let mut count = 0;
    for dx in -MIN_RADIUS..=MIN_RADIUS {
        for dz in -MIN_RADIUS..=MIN_RADIUS {
            let cell = [elite[0] + dx as i32, elite[1], elite[2] + dz as i32];
            assert!(
                standable(model, cell),
                "circle cell {cell:?} is not open ground"
            );
            count += 1;
        }
    }
    assert_eq!(count, diam * diam, "the circle is not the claimed square");
}

/// The second gate: two flank lanes, counted, not eyeballed.
#[test]
fn the_open_floor_has_two_flank_routes() {
    let out = expand_at(&elite_ground(), ARENA_REGION, ARENA_SEED);
    let elite = out.anchors["anchor/elite"].pos;
    let far_z = ARENA_REGION.size[2] as i32 - 1;
    assert_eq!(
        flank_route_count(&out.model, elite, far_z),
        2,
        "the open floor does not carry two proven bypass routes"
    );
}

/// ...and the gate has teeth. `seal_flank` walls off the west band, the east
/// band, or both, across the circle's own length — exactly the shape a
/// fog-gate motif takes — and the counted route total must drop.
#[test]
fn sealing_a_flank_drops_its_route() {
    for (knob, expected) in [(1, 1), (2, 1), (3, 0)] {
        let mut sealed = elite_ground();
        sealed.set_param("seal_flank", knob).unwrap();
        let out = expand_at(&sealed, ARENA_REGION, ARENA_SEED);
        let elite = out.anchors["anchor/elite"].pos;
        let far_z = ARENA_REGION.size[2] as i32 - 1;
        assert_eq!(
            flank_route_count(&out.model, elite, far_z),
            expected,
            "seal_flank={knob} did not drop the route count as claimed — the gate proves nothing"
        );
    }
}

/// `radius` is guarded at the entry's own stated floor: a smaller circle is a
/// refusal naming the rule, never a quietly smaller arena.
#[test]
fn a_radius_under_the_floor_is_refused_not_shrunk() {
    let mut cramped = elite_ground();
    cramped.set_param("radius", MIN_RADIUS - 1).unwrap();
    let err = expand(&cramped, ARENA_REGION, &ExpandOptions::seeded(ARENA_SEED)).unwrap_err();
    assert!(
        err.to_string().contains("no alternative of rule"),
        "expected a refusal, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// Shared promises
// ---------------------------------------------------------------------------

/// Every staging program states a smallest region it expands in. Documented
/// numbers drift; this holds them to the code from both sides.
#[test]
fn the_documented_minimum_regions_are_the_real_ones() {
    fn check(name: &str, program: &Program, smallest: [u32; 3], too_small: &[[u32; 3]]) {
        expand(
            program,
            Box3::at_origin(smallest),
            &ExpandOptions::seeded(1),
        )
        .unwrap_or_else(|e| panic!("{name} should expand at its documented minimum: {e}"));
        for &size in too_small {
            assert!(
                expand(program, Box3::at_origin(size), &ExpandOptions::seeded(1)).is_err(),
                "{name} expanded at {size:?}, below its documented minimum {smallest:?}"
            );
        }
    }
    // cliff_path: 3 across, 1 + niche_height + 1 tall, and at least as long as
    // it is wide — the rule turns its length onto the longer horizontal axis,
    // so a box narrower than 3 on its *shorter* side has no ledge to build.
    check(
        "cliff_path",
        &cliff_path(),
        [3, 4, 3],
        &[[2, 4, 3], [3, 3, 3], [3, 4, 2]],
    );
    // watch_bay: 6 across, head + 2 tall, approach + span + 4 long.
    check(
        "watch_bay",
        &watch_bay(),
        [6, 6, 15],
        &[[5, 6, 15], [6, 5, 15], [6, 6, 14]],
    );
    // rafter_hall, trussed: the density cap ties width to length, so the
    // smallest trussed hall is a point on a curve rather than a triple. At 12
    // long, 10 across is it — and one cell off either horizontal breaks the cap.
    // (Height is *not* in the list: a shorter box is the rafterless variant, not
    // an error, which `a_hall_under_six_tall_is_a_hall_without_rafters` asserts.)
    check(
        "rafter_hall",
        &rafter_hall(),
        [10, 6, 12],
        &[[9, 6, 12], [10, 6, 11]],
    );
    // ambush_door: door_offset + 5 across, head + 2 tall, and at least as long
    // as it is wide (the frame turns length onto the longer horizontal).
    check(
        "ambush_door",
        &ambush_door(),
        [7, 5, 7],
        &[[6, 5, 7], [7, 4, 7], [7, 5, 6]],
    );
    // store_room: 5 across (wall, floor, barrels, wall — and a floor worth
    // standing on), 5 tall (floor, barrels, headroom, ceiling), 3 of row.
    check(
        "store_room",
        &store_room(),
        [5, 5, 5],
        &[[4, 5, 5], [5, 4, 5], [5, 5, 4]],
    );
    // boulder_stair: MIN_X (5) across — and at least as long as it is wide,
    // which for this rule means at least MIN_X deep too: the frame always
    // makes local Z the *larger* of the two horizontal extents, so a
    // documented depth under the width minimum could never be reached.
    check(
        "boulder_stair",
        &boulder_stair(),
        [
            boulder_stair::MIN_X as u32,
            6,
            boulder_stair::MIN_DEPTH as u32,
        ],
        &[[4, 6, 5], [5, 5, 5], [5, 6, 4]],
    );
    // threshold_motif: 3 across (wall, interior, wall), head + 2 (6) tall —
    // head defaults to curtain_height + 2, the least that leaves two full
    // cells of walk clearance under the curtain — 3 long.
    check(
        "threshold_motif",
        &threshold_motif(),
        [3, 6, threshold_motif::MIN_DEPTH as u32],
        &[[2, 6, 3], [3, 5, 3], [3, 6, 2]],
    );
    // broken_grate: 3 across (wall, floor, grate wall), head + 2 tall,
    // MIN_LINE (3) — three grates is the shortest row the odd one always has
    // a neighbour in, the same proof store_room makes for its barrels.
    check(
        "broken_grate",
        &broken_grate(),
        [3, 5, broken_grate::MIN_LINE as u32],
        &[[2, 5, 3], [3, 4, 3], [3, 5, 2]],
    );
    // drop_shaft: 3 across, drop + head + 1 tall, 4 long (2 cells per zone).
    check(
        "drop_shaft",
        &drop_shaft(),
        [3, 8, 4],
        &[[2, 8, 4], [3, 7, 4], [3, 8, 3]],
    );
    // dumbwaiter: duct_width + 4 across, drop + head + 1 tall, duct_len + 4 long.
    check(
        "dumbwaiter",
        &dumbwaiter(),
        [5, 8, 6],
        &[[4, 8, 6], [5, 7, 6], [5, 8, 5]],
    );
    // far_side_bar: 3 across, head + 2 tall, 3 long (a cell of floor on each
    // side of the wall).
    check(
        "far_side_bar",
        &far_side_bar(),
        [3, 5, 3],
        &[[2, 5, 3], [3, 4, 3], [3, 5, 2]],
    );
    // tee_passage: MIN_WIDTH (3) across (door wall, lane, side wall), head + 2
    // tall, MIN_LENGTH (3) long — a cell of wall on each side of the doorway,
    // so the opening is a doorway and not the whole face.
    check(
        "tee_passage",
        &tee_passage(),
        [
            tee_passage::MIN_WIDTH as u32,
            5,
            tee_passage::MIN_LENGTH as u32,
        ],
        &[[2, 5, 3], [3, 4, 3], [3, 5, 2]],
    );
    // causeway: 5 across (wall, flood, causeway, flood, wall), rise +
    // tower_rise + head tall, guard_len + 3 long (guard_len's own default is 2).
    check(
        "causeway",
        &causeway(),
        [5, 10, 5],
        &[[4, 10, 5], [5, 9, 5], [5, 10, 4]],
    );
    // stair_flight: MIN_WIDTH (3) across (wall, lane, wall), head + 1 +
    // MIN_STEPS tall, and 2*landing_run + MIN_STEPS*tread long — the box has to
    // hold three treads, or the rule refuses rather than laying a doorstep.
    check(
        "stair_flight",
        &stair_flight(),
        [stair_flight::MIN_WIDTH as u32, 7, 12],
        &[[2, 7, 12], [3, 6, 12], [3, 7, 11]],
    );
    // elite_ground: both horizontal extents >= diameter + 2*flank_margin + 2
    // (the wider of the rule's two checks — see the module note), head + 2
    // tall — at `radius`'s enforced floor of 4 (a 9×9 circle) and the default
    // margin/approach/head.
    check(
        "elite_ground",
        &elite_ground(),
        [19, 5, 19],
        &[[18, 5, 19], [19, 4, 19], [19, 5, 18]],
    );
}

/// A palette swap restyles every staging program without moving a block — the
/// same promise the ported library makes, asserted for the original rules too.
///
/// Every role of every program, not one hand-picked role each: `store_room`'s
/// whole point is that a campaign can restyle the barrels *and* the tell and
/// still have a tell, which is only true if both roles are style and neither is
/// geometry.
#[test]
fn the_staging_rules_restyle_without_moving_a_block() {
    const SWATCH: &[&str] = &[
        "deepslate_bricks",
        "polished_blackstone",
        "cracked_nether_bricks",
        "warped_planks",
    ];
    for (base, region) in [
        (cliff_path(), CLIFF_REGION),
        (watch_bay(), PASSAGE_REGION),
        (rafter_hall(), HALL_REGION),
        (ambush_door(), DOOR_REGION),
        (store_room(), STORE_REGION),
        (boulder_stair(), STAIR_REGION),
        (threshold_motif(), THRESHOLD_REGION),
        (broken_grate(), GRATE_REGION),
        (drop_shaft(), SHAFT_REGION),
        (dumbwaiter(), DUCT_REGION),
        (far_side_bar(), BAR_REGION),
        (tee_passage(), TEE_REGION),
        (causeway(), CAUSEWAY_REGION),
        (elite_ground(), ARENA_REGION),
        (stair_flight(), FLIGHT_REGION),
        (hearth_ward(), HEARTH_REGION),
        (bait_stand(), BAIT_REGION),
        (disarm_stand(), DISARM_REGION),
    ] {
        let mut restyled = base.clone();
        let roles: Vec<String> = base.palette.keys().cloned().collect();
        assert!(!roles.is_empty(), "{} binds no roles", base.name);
        for (i, role) in roles.iter().enumerate() {
            restyled
                .set_role(
                    role,
                    delvewright_grammar::ir::Paint::Block(BlockState::simple(
                        SWATCH[i % SWATCH.len()],
                    )),
                )
                .unwrap();
        }
        let plain = expand_at(&base, region, 3);
        let dark = expand_at(&restyled, region, 3);
        assert_eq!(
            plain.model.filled_cells(),
            dark.model.filled_cells(),
            "{} moved a block when it was restyled",
            base.name
        );
        assert_eq!(plain.anchors, dark.anchors, "{}", base.name);
        assert_ne!(
            plain.model.canonical_bytes(),
            dark.model.canonical_bytes(),
            "{}'s restyle changed nothing — the roles do not reach the blocks",
            base.name
        );
    }
}

// ---------------------------------------------------------------------------
// The rest point, the lure and the control (`hearth_ward`, `bait_stand`,
// `disarm_stand`) — the three mechanisms the zone round needed and the
// vocabulary did not have
// ---------------------------------------------------------------------------

/// The hearth-ward fixture: a lane with room for a nook beside it, and length
/// enough that the nook's band has plain corridor at both ends of it. `Z`
/// strictly longer than `X`, as `SHAFT_TEETH_REGION` notes.
const HEARTH_REGION: Box3 = Box3::at_origin([8, 6, 14]);
/// `hearth_ward` draws nothing from the seed; it is stated, not chosen.
const HEARTH_SEED: u64 = 1;

/// The bait-stand fixture, and two more box shapes the co-location gate is
/// re-bound over: a motif that only lines up at one width is a coincidence.
const BAIT_REGION: Box3 = Box3::at_origin([9, 8, 14]);
const BAIT_SEED: u64 = 1;
const BAIT_SIZES: [Box3; 3] = [
    Box3::at_origin([9, 8, 14]),
    Box3::at_origin([7, 8, 12]),
    Box3::at_origin([13, 9, 20]),
];

/// The disarm-stand fixture: a head with room for the stand beside the lane,
/// and a run long enough to be worth jamming.
const DISARM_REGION: Box3 = Box3::at_origin([9, 7, 16]);
/// `disarm_stand` draws nothing from the seed; it is stated, not chosen.
const DISARM_SEED: u64 = 1;

/// The standable cells at each end of a piece's travel axis.
fn travel_ends(model: &VoxelModel) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let region = model.region();
    let far = region.origin[2] + region.size[2] as i32 - 1;
    let near = region.origin[2];
    let cells = standable_cells(model);
    (
        cells.iter().copied().filter(|c| c[2] == far).collect(),
        cells.iter().copied().filter(|c| c[2] == near).collect(),
    )
}

/// The nook's own standable cells, read off the anchor inside it and the rule's
/// published width rather than recomputed from the fixture's arithmetic.
///
/// `anchor/hearth` sits at the floor centre of the nook's inner half, and a
/// two-wide box centres onto its lower cell, so the anchor's own `x` is the
/// nook's `X`-min and its `z` is one past the mouth.
fn nook_cells(out: &Expansion, nook_len: i32) -> BTreeSet<[i32; 3]> {
    let hearth = out.anchors["anchor/hearth"].pos;
    standable_cells(&out.model)
        .into_iter()
        .filter(|c| {
            c[0] >= hearth[0]
                && c[0] < hearth[0] + hearth_ward::NOOK_WIDTH as i32
                && c[2] >= hearth[2] - 1
                && c[2] < hearth[2] - 1 + nook_len
        })
        .collect()
}

/// Every standable cell outside `set` that a walker could step into it from.
fn neighbours_of(cells: &BTreeSet<[i32; 3]>, set: &BTreeSet<[i32; 3]>) -> BTreeSet<[i32; 3]> {
    let mut found = BTreeSet::new();
    for [x, y, z] in set.iter().copied() {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            for dy in [0, 1, -1] {
                let next = [x + dx, y + dy, z + dz];
                if cells.contains(&next) && !set.contains(&next) {
                    found.insert(next);
                }
            }
        }
    }
    found
}

/// Gate 1: a rest ward is a **chain segment**, not a room. The lane walks end to
/// end, so a zone that lays one in its piece run still has a route through it.
///
/// Binding: 78 standable cells, 6 at each end.
#[test]
fn the_hearth_ward_is_a_lane_that_walks_end_to_end() {
    let out = expand_at(&hearth_ward(), HEARTH_REGION, HEARTH_SEED);
    let cells = standable_cells(&out.model);
    assert_eq!(cells.len(), 78);
    let (entry, exit) = travel_ends(&out.model);
    assert_eq!(entry.len(), 6, "the lane at the approach end");
    assert_eq!(exit.len(), 6, "the lane at the way out");
    assert!(
        connected(&cells, &entry, &exit),
        "the rest ward does not walk end to end"
    );
}

/// Gate 2: the focus is reachable, and reaching it is a **detour**. A rest point
/// on the road is a corridor with a campfire in it; the whole mechanism is that
/// you step aside for it.
///
/// Binding: 6 nook cells deleted, 78 standable re-walked without them. Teeth:
/// `mouth_sealed`.
#[test]
fn the_hearth_is_reachable_and_off_the_road() {
    let out = expand_at(&hearth_ward(), HEARTH_REGION, HEARTH_SEED);
    let cells = standable_cells(&out.model);
    let hearth: BTreeSet<[i32; 3]> = [out.anchors["anchor/hearth"].pos].into_iter().collect();
    assert!(
        standable(&out.model, out.anchors["anchor/hearth"].pos),
        "nothing can rest at {:?}",
        out.anchors["anchor/hearth"].pos
    );
    let (entry, exit) = travel_ends(&out.model);
    assert!(
        connected(&cells, &entry, &hearth),
        "the lane cannot reach its own hearth"
    );

    let nook = nook_cells(&out, 3);
    assert_eq!(nook.len(), 6, "the nook's own cells");
    let without: BTreeSet<[i32; 3]> = cells.difference(&nook).copied().collect();
    assert!(
        connected(&without, &entry, &exit),
        "delete the nook and the lane stops walking — the rest point is on the \
         road rather than beside it"
    );
}

/// ...and it has teeth: filling the nook's one open cell leaves the hearth
/// standing and unreachable, while the lane walks exactly as before.
#[test]
fn sealing_the_nooks_mouth_puts_the_hearth_out_of_reach() {
    let mut sealed = hearth_ward();
    sealed.set_param("mouth_sealed", 1).unwrap();
    let out = expand_at(&sealed, HEARTH_REGION, HEARTH_SEED);
    let cells = standable_cells(&out.model);
    assert_eq!(
        cells.len(),
        76,
        "the mouth's two cells, and only they, are gone"
    );
    let hearth: BTreeSet<[i32; 3]> = [out.anchors["anchor/hearth"].pos].into_iter().collect();
    let (entry, exit) = travel_ends(&out.model);
    assert!(
        !connected(&cells, &entry, &hearth),
        "the mouth was filled and the hearth is still reachable — the way in was \
         never the mouth"
    );
    assert!(
        connected(&cells, &entry, &exit),
        "sealing the nook sealed the lane, so the red above is measuring a \
         severed ward rather than an unreachable hearth"
    );
}

/// Gate 3: **exactly one way in.** What makes a rest point defensible is that
/// you can only be come at from the direction you are already facing, and that
/// is a count of the nook's standable neighbours, not a claim about walls.
///
/// Binding: 6 nook cells, 2 neighbours. Teeth: `back_door`.
#[test]
fn the_nook_has_exactly_one_way_in() {
    let out = expand_at(&hearth_ward(), HEARTH_REGION, HEARTH_SEED);
    let cells = standable_cells(&out.model);
    let nook = nook_cells(&out, 3);
    assert_eq!(nook.len(), 6);
    let ways_in = neighbours_of(&cells, &nook);
    assert_eq!(
        ways_in.len(),
        hearth_ward::NOOK_WIDTH as usize,
        "the nook is approachable from {ways_in:?} — a rest point with a second \
         approach is a room, not a shelter"
    );
    // ...and the one way in is the mouth: every neighbour is on the lane side of
    // the nook's own Z-min face.
    let hearth = out.anchors["anchor/hearth"].pos;
    for cell in &ways_in {
        assert_eq!(
            cell[2],
            hearth[2] - 2,
            "{cell:?} is not in front of the mouth"
        );
    }
}

/// ...and it has teeth: `back_door` opens the outer wall behind the nook, and
/// the count of ways in rises off the mouth's own two.
#[test]
fn a_door_behind_the_hearth_reds_the_shelter_gate() {
    let mut holed = hearth_ward();
    holed.set_param("back_door", 1).unwrap();
    let out = expand_at(&holed, HEARTH_REGION, HEARTH_SEED);
    let cells = standable_cells(&out.model);
    let nook = nook_cells(&out, 3);
    assert_eq!(nook.len(), 6, "the nook itself did not change");
    let ways_in = neighbours_of(&cells, &nook);
    assert_eq!(
        ways_in.len(),
        5,
        "a doorway was opened behind the hearth and the shelter gate still \
         counted one way in — it proves nothing"
    );
}

/// A box too narrow to hold a nook beside a lane is a refusal naming the rule,
/// never a rest ward that quietly is not one.
///
/// Binding: 1 refusal, against the same box one cell wider, which builds.
#[test]
fn a_ward_too_narrow_for_a_nook_is_refused() {
    let program = hearth_ward();
    let narrow = Box3::at_origin([hearth_ward::MIN_WIDTH as u32 - 1, 6, 14]);
    let err = expand(&program, narrow, &ExpandOptions::seeded(HEARTH_SEED)).unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("ward_plan"),
        "the refusal does not name the rule that refused: {text}"
    );
    expand_at(
        &program,
        Box3::at_origin([hearth_ward::MIN_WIDTH as u32, 6, 14]),
        HEARTH_SEED,
    );
}

/// Gate 1 of the lure: **the watcher stands over the bait.** Same column, the
/// perch above, the perch standable and the pedestal a solid block a campaign
/// can put something on.
///
/// Binding: 3 box shapes, each one bait/perch pair.
#[test]
fn the_watcher_stands_directly_over_the_lure() {
    for region in BAIT_SIZES {
        let out = expand_at(&bait_stand(), region, BAIT_SEED);
        let bait = out.anchors["anchor/bait"].pos;
        let perch = out.anchors["anchor/bait-perch"].pos;
        assert_eq!(
            [bait[0], bait[2]],
            [perch[0], perch[2]],
            "{region:?}: the perch is not over the pedestal"
        );
        assert!(
            perch[1] > bait[1] + 1,
            "{region:?}: the perch is on the pedestal rather than over it"
        );
        assert!(
            solid(&out.model, bait),
            "{region:?}: the pedestal is not a block"
        );
        assert!(
            standable(&out.model, perch),
            "{region:?}: nothing can wait on the perch"
        );
        // The display space: the cell over the pedestal is open, or there is
        // nowhere to put the lure at all.
        assert!(
            passable(&out.model, [bait[0], bait[1] + 1, bait[2]]),
            "{region:?}: the pedestal has no room over it"
        );
    }
}

/// Gate 2, and the reason this rule exists: **wherever the lure can be seen
/// from, so can the watcher.** That is variant 1 of the dossier's bait pattern
/// stated as geometry; variant 3 — the trigger you cannot see — is banned, and
/// this gate is what makes the ban machine-checkable.
///
/// Binding: 42 approach cells, all 42 of which see the bait, and all 42 the
/// perch. Teeth: `canopy`.
#[test]
fn the_watcher_is_visible_wherever_the_lure_is() {
    let out = expand_at(&bait_stand(), BAIT_REGION, BAIT_SEED);
    let bait = out.anchors["anchor/bait"].pos;
    let perch = out.anchors["anchor/bait-perch"].pos;
    let approach: Vec<[i32; 3]> = standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[2] > perch[2] + 1)
        .collect();
    assert_eq!(approach.len(), 42, "the approach the player decides from");
    let mut lure_seen = 0;
    for cell in &approach {
        if sees(&out.model, *cell, bait).is_ok() {
            lure_seen += 1;
            if let Err(blocker) = sees(&out.model, *cell, perch) {
                panic!(
                    "from {cell:?} the lure is visible and the body over it is not \
                     ({blocker:?} is in the way) — that is the displaced ambush the \
                     catalogue bans"
                );
            }
        }
    }
    assert_eq!(lure_seen, 42, "the lure is visible from the whole approach");
}

/// ...and it has teeth: `canopy` hangs a valance in front of the perch. The
/// lure's own count does not move — which is what makes the red an *ambush*
/// defect and not a walled-off room.
#[test]
fn a_canopy_over_the_lure_hides_its_watcher() {
    let mut hidden = bait_stand();
    hidden.set_param("canopy", 1).unwrap();
    let out = expand_at(&hidden, BAIT_REGION, BAIT_SEED);
    let bait = out.anchors["anchor/bait"].pos;
    let perch = out.anchors["anchor/bait-perch"].pos;
    let approach: Vec<[i32; 3]> = standable_cells(&out.model)
        .into_iter()
        .filter(|c| c[2] > perch[2] + 1)
        .collect();
    assert_eq!(approach.len(), 42, "the same approach set");
    let lure_seen = approach
        .iter()
        .filter(|c| sees(&out.model, **c, bait).is_ok())
        .count();
    let watcher_seen = approach
        .iter()
        .filter(|c| sees(&out.model, **c, perch).is_ok())
        .count();
    assert_eq!(lure_seen, 42, "the valance moved the lure's own visibility");
    assert_eq!(
        watcher_seen, 0,
        "a valance was hung in front of the perch and the co-visibility gate \
         still read it as legible — the gate proves nothing"
    );
}

/// Gate 3: the gallery is a chain segment too, so a zone can lay one in its
/// piece run.
///
/// Binding: 99 standable cells, 5 at each end.
#[test]
fn the_bait_stand_is_a_room_that_walks_end_to_end() {
    let out = expand_at(&bait_stand(), BAIT_REGION, BAIT_SEED);
    let cells = standable_cells(&out.model);
    assert_eq!(cells.len(), 99);
    let (entry, exit) = travel_ends(&out.model);
    assert_eq!(entry.len(), 7);
    assert_eq!(exit.len(), 7);
    assert!(
        connected(&cells, &entry, &exit),
        "the gallery does not walk"
    );
}

/// A perch with no air over it is not a perch, and the rule refuses rather than
/// hanging one in the ceiling.
///
/// Binding: 2 refusals (`perch_rise` under its floor, and a `head` that does not
/// clear the perch), against the defaults, which build.
#[test]
fn a_perch_with_no_air_over_it_is_refused() {
    for (knob, value) in [("perch_rise", bait_stand::MIN_RISE - 1), ("head", 4)] {
        let mut bad = bait_stand();
        bad.set_param(knob, value).unwrap();
        let err = expand(&bad, BAIT_REGION, &ExpandOptions::seeded(BAIT_SEED)).unwrap_err();
        let text = err.to_string();
        assert!(
            text.contains("stand_plan"),
            "{knob}={value}: the refusal does not name the rule: {text}"
        );
    }
    expand_at(&bait_stand(), BAIT_REGION, BAIT_SEED);
}

/// Gate 1 of the control: the lane still walks, so a stand at the head of a
/// hazard does not become the thing that severs the zone.
///
/// Binding: 107 standable cells.
#[test]
fn the_disarm_stand_is_a_lane_that_walks_end_to_end() {
    let out = expand_at(&disarm_stand(), DISARM_REGION, DISARM_SEED);
    let cells = standable_cells(&out.model);
    assert_eq!(cells.len(), 107);
    let (entry, exit) = travel_ends(&out.model);
    assert_eq!(entry.len(), 4, "the lane past the stand");
    assert_eq!(exit.len(), 7, "the run at the far end");
    assert!(
        connected(&cells, &entry, &exit),
        "the head does not join its run"
    );
}

/// Gate 2, and the whole point of the mechanism: **the release cannot be worked
/// from the run it governs.** Every standable cell of the run is checked for
/// adjacency to the mechanism, so the claim binds to the run's own size and
/// cannot go quietly vacuous on a shorter box.
///
/// Binding: 103 run cells examined, 0 of them in reach of the release. Teeth:
/// `release_in_lane`.
#[test]
fn the_release_is_out_of_reach_of_its_own_run() {
    let out = expand_at(&disarm_stand(), DISARM_REGION, DISARM_SEED);
    let release = out.anchors["anchor/release"].pos;
    assert!(
        solid(&out.model, release),
        "the release is not a block anything could be bound to"
    );
    let (run, operators) = run_and_operators(&out);
    assert_eq!(run.len(), 103, "the run the release governs");
    let from_run = run.iter().filter(|c| adjacent(**c, release)).count();
    assert_eq!(
        from_run, 0,
        "the release can be worked from inside the run it governs — a hazard you \
         disarm while standing in it is not a third rung"
    );
    assert_eq!(operators.len(), 1, "the stand's own operating position");
}

/// ...and it has teeth: `release_in_lane` sets the mechanism into the divider
/// instead of the outer wall, which is exactly the mistake, and the count of
/// in-run operating positions rises off zero.
#[test]
fn a_release_in_the_divider_reds_the_out_of_reach_gate() {
    let mut wrong = disarm_stand();
    wrong.set_param("release_in_lane", 1).unwrap();
    let out = expand_at(&wrong, DISARM_REGION, DISARM_SEED);
    let release = out.anchors["anchor/release"].pos;
    let (run, _) = run_and_operators(&out);
    assert_eq!(run.len(), 103, "the same run");
    let from_run = run.iter().filter(|c| adjacent(**c, release)).count();
    assert_eq!(
        from_run, 1,
        "the mechanism was moved into the lane's own wall and the gate still \
         reported it out of reach — it proves nothing"
    );
}

/// Gate 3: ...and it can be worked at all. A control nobody can reach is not
/// safer than one in the lane, it is absent.
///
/// Binding: 1 operating position, reached from the run's 103 cells. Teeth:
/// `stand_sealed`.
#[test]
fn the_release_can_be_reached_from_the_run() {
    let out = expand_at(&disarm_stand(), DISARM_REGION, DISARM_SEED);
    let cells = standable_cells(&out.model);
    let (run, operators) = run_and_operators(&out);
    assert_eq!(operators.len(), 1);
    assert!(
        connected(&cells, &run, &operators),
        "the stand cannot be entered from the run — the release is unreachable"
    );

    let mut sealed = disarm_stand();
    sealed.set_param("stand_sealed", 1).unwrap();
    let shut = expand_at(&sealed, DISARM_REGION, DISARM_SEED);
    let shut_cells = standable_cells(&shut.model);
    let (shut_run, shut_operators) = run_and_operators(&shut);
    assert_eq!(
        shut_operators.len(),
        1,
        "the operating position is still standable, merely cut off"
    );
    assert!(
        !connected(&shut_cells, &shut_run, &shut_operators),
        "the stand's mouth was filled and the release is still reachable — the \
         way in was never the mouth"
    );
    let (entry, exit) = travel_ends(&shut.model);
    assert!(
        connected(&shut_cells, &entry, &exit),
        "sealing the stand sealed the lane, so the red above is measuring a \
         severed piece rather than an unreachable control"
    );
}

/// The run the release governs, and the positions it can be worked from.
///
/// The run is **every standable cell but the stand's own**, deliberately: the
/// hazard's path is the whole lane, including the stretch of it that runs past
/// the head, and a definition that only counted the far zone would let a
/// mechanism reachable from the lane beside the stand pass unnoticed.
fn run_and_operators(out: &Expansion) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let release = out.anchors["anchor/release"].pos;
    let head = out.anchors["anchor/run-head"].pos;
    let origin = out.model.region().origin;
    let stand: BTreeSet<[i32; 3]> = standable_cells(&out.model)
        .into_iter()
        .filter(|c| {
            c[0] > origin[0]
                && c[0] <= origin[0] + disarm_stand::STAND_WIDTH as i32
                && c[2] > head[2]
        })
        .collect();
    let run: BTreeSet<[i32; 3]> = standable_cells(&out.model)
        .into_iter()
        .filter(|c| !stand.contains(c))
        .collect();
    let operators: BTreeSet<[i32; 3]> = standable_cells(&out.model)
        .into_iter()
        .filter(|c| adjacent(*c, release))
        .filter(|c| stand.contains(c))
        .collect();
    (run, operators)
}

/// Orthogonally touching, at the same height: what "in reach" means for a hand
/// on a wall block.
fn adjacent(cell: [i32; 3], block: [i32; 3]) -> bool {
    cell[1] == block[1] && (cell[0] - block[0]).abs() + (cell[2] - block[2]).abs() == 1
}

/// A box with no room for a stand beside the lane is a refusal naming the rule.
///
/// Binding: 1 refusal, against the same box one cell wider, which builds.
#[test]
fn a_head_too_narrow_for_a_stand_is_refused() {
    let program = disarm_stand();
    let narrow = Box3::at_origin([disarm_stand::MIN_WIDTH as u32 - 1, 7, 16]);
    let err = expand(&program, narrow, &ExpandOptions::seeded(DISARM_SEED)).unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("stand_plan"),
        "the refusal does not name the rule that refused: {text}"
    );
    expand_at(
        &program,
        Box3::at_origin([disarm_stand::MIN_WIDTH as u32, 7, 16]),
        DISARM_SEED,
    );
}
