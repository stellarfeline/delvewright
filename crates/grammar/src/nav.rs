//! Reading an expanded model the way a body meets it: what it can stand in, and
//! what it can walk to.
//!
//! # Why this is library code
//!
//! These predicates were written inside `tests/`, where they answered one
//! question per rule ("is `cliff_path`'s ledge the only route?"). That is the
//! right place for a rule's own gate and the wrong place for the predicate the
//! gate is written in: a program authored *outside* this repo — the whole point
//! of spec-0027, where the model writes the rules — has no `tests/support` to
//! reach for, so its author would have no way to ask whether the piece can be
//! walked at all. A capability belongs to the object it acts on (CLAUDE.md): a
//! walk over a [`VoxelModel`] belongs to the crate that defines `VoxelModel`.
//!
//! `tests/support/mod.rs` now delegates here. `tests/staging.rs` still carries
//! its own copy — that file's own header records why (it landed first and two
//! vocabulary families are in review against it), and folding it in is a
//! follow-up, not a silence.
//!
//! # What the model is, and is not
//!
//! One walker: one cell horizontally at a time, stepping at most one block up or
//! down, and — under [`reachable_with_fall`] only — walking off a ledge and
//! landing on the first floor below. **No jump.** Every "cannot reach" these
//! functions prove means *by walking*, which is the conservative direction for a
//! severing claim and the generous one for a reachability claim, so the two are
//! never interchangeable.

use std::collections::{BTreeSet, VecDeque};

use crate::model::VoxelModel;

/// Cells a body and a sightline pass through.
///
/// Everything the rule library places is a full block except air and a floor
/// skull, which is neither a barrier nor an occluder — and which sits on the
/// exact cell an anchor names, so a naive "not air means solid" predicate would
/// report that niche unreachable. Outside the region counts as blocking: a body
/// that has left the model has left the thing being proved.
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
/// Deliberately *more* permissive than [`connected`], which is why the two are
/// used in opposite directions and never interchangeably: forward, where a piece
/// is entered by stepping off a ledge and `connected` alone would call it
/// severed; and as the adversary, when a piece claims a route is the only route,
/// since a fall edge can carry a body past a wall a walker would have to go
/// round. The negative direction — "there is no way back up" — is asserted under
/// the plain walk instead, because a fall edge only ever points down and proving
/// a negative under the generous model would be circular.
///
/// A landing must be a member of `cells`, not merely standable in the model: a
/// gate proves a route is the only route by deleting cells from the graph, and a
/// fall allowed to land on a deleted cell would walk straight through the cut.
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

/// The standable cells at each end of the model's local travel axis: the entry
/// (world `Z`-max, where the player comes in) and the exit (`Z`-min).
///
/// The convention is the rule library's own frame (`docs/reference/grammar.md`
/// §5b): local `Z`-max is the approach end and travel runs toward `Z`-min.
pub fn ends(model: &VoxelModel) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let region = model.region();
    let far = region.origin[2] + region.size[2] as i32 - 1;
    let near = region.origin[2];
    let cells = standable_cells(model);
    let entry = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit = cells.iter().copied().filter(|c| c[2] == near).collect();
    (entry, exit)
}
