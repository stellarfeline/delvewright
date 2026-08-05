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
use delvewright_grammar::library::rafter_hall::FLOOR_CELLS_PER_PERCH;
use delvewright_grammar::library::{ambush_door, cliff_path, rafter_hall, store_room, watch_bay};
use delvewright_grammar::{Anchor, Box3, ExpandOptions, Expansion, VoxelModel, expand};

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

// ---------------------------------------------------------------------------
// Reading the expanded model the way a player meets it
// ---------------------------------------------------------------------------

/// Cells a body and a sightline pass through.
///
/// The grammar's terminals in these two programs are stone and a floor skull.
/// A skull is neither a barrier nor an occluder, and saying so matters: the
/// teaching niche has one on the exact cell its anchor names, so a naive
/// "not air means solid" predicate would report that niche as unreachable and
/// invisible. Outside the region counts as blocking — a ray that leaves the
/// prefab has left the thing being proved.
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
        let inward = if x < HALL_REGION.size[0] as i32 / 2 { -1 } else { 1 };
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
            .filter(|&p| {
                model
                    .get(p)
                    .is_some_and(|b| b.name.starts_with(TELL_BLOCK))
            })
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
