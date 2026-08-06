//! Reading an expanded model the way a player meets it: what a body can stand
//! in, what a walker can reach, and what an eye can see.
//!
//! The sightline walk is the compiler's: eye at 1.62 over the watch cell, target
//! at 1.0 over the observed cell, Amanatides–Woo cell traversal, both endpoint
//! cells exempt (`crates/compiler/src/nav.rs`, which `DW0388` uses). The
//! generator's idea of a sightline and the compiler's must not drift apart, so
//! the shape is copied rather than reinvented.
//!
//! `tests/staging.rs` carries its own copy of these helpers: it landed first,
//! and two more vocabulary families are in review against that file right now.
//! Folding it onto this module is a follow-up that costs nothing to do later and
//! would cost every one of those PRs a conflict to do now.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_grammar::ir::Program;
use delvewright_grammar::{Anchor, Box3, ExpandOptions, Expansion, VoxelModel, expand};

/// Player eye height over the cell they stand in (vanilla 1.62), as `DW0388`.
const EYE_HEIGHT: f64 = 1.62;

/// Expand, or fail naming the program and the error.
pub fn expand_at(program: &Program, region: Box3, seed: u64) -> Expansion {
    expand(program, region, &ExpandOptions::seeded(seed))
        .unwrap_or_else(|e| panic!("{}: {e}", program.name))
}

/// Cells a body and a sightline pass through.
///
/// Everything the staging vocabulary places is a full block except air and the
/// teaching niche's floor skull, which is neither a barrier nor an occluder —
/// and which sits on the exact cell an anchor names, so a naive "not air means
/// solid" predicate would report that niche unreachable and invisible. Outside
/// the region counts as blocking: a ray that has left the model has left the
/// thing being proved.
pub fn passable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    match model.get(pos) {
        None => false,
        Some(block) => block.is_air() || block.name.ends_with("_skull"),
    }
}

/// A full block: what a floor is made of, and what stops an eye.
pub fn solid(model: &VoxelModel, pos: [i32; 3]) -> bool {
    model.get(pos).is_some() && !passable(model, pos)
}

/// A cell a player can stand in: two blocks of clearance over a full floor.
pub fn standable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    let [x, y, z] = pos;
    passable(model, pos) && passable(model, [x, y + 1, z]) && solid(model, [x, y - 1, z])
}

/// Every standable cell of the model.
pub fn standable_cells(model: &VoxelModel) -> BTreeSet<[i32; 3]> {
    model
        .region()
        .positions()
        .filter(|&p| standable(model, p))
        .collect()
}

/// Can a walker get from any cell of `from` to any cell of `to`, moving one cell
/// horizontally at a time and stepping at most one block up or down?
pub fn connected(
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

/// [`connected`]'s ±1-step walk, plus a one-way **fall**: stepping off a
/// standable cell into an adjacent column with nothing underfoot, and landing on
/// the first solid floor below however far that is.
///
/// This is the player's own movement model, and it is deliberately *more*
/// permissive than [`connected`] — which is why it is used in two opposite
/// directions and never interchangeably with it:
///
/// * **forward**, where a zone contains one-way hardware (Z6 is entered by
///   stepping off a ledge, so `connected` alone would call the zone severed);
/// * **as the adversary**, when a zone claims a route is the only route. A fall
///   edge can carry a player *past* a wall that a walker would have to go
///   round, so a severing cut is only proved severing if it survives this model.
///
/// The negative direction — "there is no way back up" — is asserted under the
/// plain [`connected`] walk instead, because a fall edge only ever points down
/// and proving a negative under the model built to be generous would be
/// circular. `tests/staging.rs` carries the same function and the same
/// reasoning at piece scale.
///
/// **What it does not model: a horizontal jump.** Every "unreachable" it proves
/// means unreachable *by walking and falling*. For the zones here the
/// consequence is benign in both directions: a jump cannot lift a player out of
/// the cistern, and it could only ever add flank routes to an arena.
///
/// One deliberate divergence from the piece-scale copy: a landing must be a
/// member of `cells`, not merely standable in the model. A zone gate proves a
/// route is the only route by deleting cells from the graph, and a fall that
/// was allowed to land on a deleted cell would walk straight through the cut.
pub fn reachable_with_fall(
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
            // The fall: walk the adjacent column down from foot height until it
            // hits solid ground, and land there if that landing is standable and
            // is one of the cells under consideration. However far the drop,
            // this only ever adds an edge downward.
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
                        if cells.contains(&landing) && seen.insert(landing) {
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

/// Whether an observer standing on `watch` can see the space a body standing on
/// `target` occupies. Both endpoint cells are exempt: the observer's own head,
/// and the volume being looked at.
pub fn sees(model: &VoxelModel, watch: [i32; 3], target: [i32; 3]) -> Result<(), [i32; 3]> {
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
/// the first for which `hit` holds. Amanatides–Woo voxel traversal.
pub fn walk_cells(a: [f64; 3], b: [f64; 3], hit: impl Fn([i32; 3]) -> bool) -> Option<[i32; 3]> {
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

/// Anchors whose exported name starts with `anchor/<stem>-`, in index order.
pub fn indexed(anchors: &BTreeMap<String, Anchor>, stem: &str) -> Vec<[i32; 3]> {
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

/// The standable cells at each end of a zone's travel axis: the entry (local
/// `Z`-max, where the player comes in) and the exit (`Z`-min).
pub fn ends(model: &VoxelModel) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let region = model.region();
    let far = region.origin[2] + region.size[2] as i32 - 1;
    let near = region.origin[2];
    let cells = standable_cells(model);
    let entry = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit = cells.iter().copied().filter(|c| c[2] == near).collect();
    (entry, exit)
}
