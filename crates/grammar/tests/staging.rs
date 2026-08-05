//! The W1 staging vocabulary: the knockback niche (`cliff_path`) and the watch
//! bay (`watch_bay`).
//!
//! These two rules exist for their **gates**, not for their prettiness. A niche
//! is only a knockback test if the recess is shallow enough to swing into and
//! the ledge is the only way past; a bay is only observability hardware if you
//! can actually see the hazard from it. Both claims are geometry, so both are
//! asserted here against the expanded model rather than described in prose.
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
use delvewright_grammar::library::{cliff_path, watch_bay};
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

/// Both programs state a smallest region they expand in. Documented numbers
/// drift; this holds them to the code from both sides.
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
}

/// A palette swap restyles both programs without moving a block — the same
/// promise the ported library makes, asserted for the original rules too.
#[test]
fn the_staging_rules_restyle_without_moving_a_block() {
    for (mut program, base, region) in [
        (cliff_path(), cliff_path(), CLIFF_REGION),
        (watch_bay(), watch_bay(), PASSAGE_REGION),
    ] {
        let role = if program.palette.contains_key("rock") {
            "rock"
        } else {
            "stone"
        };
        program
            .set_role(
                role,
                delvewright_grammar::ir::Paint::Block(BlockState::simple("deepslate_bricks")),
            )
            .unwrap();
        let plain = expand_at(&base, region, 3);
        let dark = expand_at(&program, region, 3);
        assert_eq!(plain.model.filled_cells(), dark.model.filled_cells());
        assert_eq!(plain.anchors, dark.anchors);
        assert_ne!(plain.model.canonical_bytes(), dark.model.canonical_bytes());
    }
}
