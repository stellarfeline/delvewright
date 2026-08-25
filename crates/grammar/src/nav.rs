//! Reading an expanded model the way a body meets it: what it can stand in, and
//! what it can walk to.
//!
//! # Where the walk lives
//!
//! The walk itself is not here. It is [`delvewright_schem::nav`], because "can a
//! body stand on this cell, and can it walk from that one to this one" is the
//! same question over a grammar expansion, over a structure template read off
//! disk, and over a zone reassembled from tiles — one question, one
//! implementation, and a fix to it reaches every caller. A capability belongs to
//! the object class it acts on (CLAUDE.md), and that class is *a box of cells
//! with a passability answer for each*, not `VoxelModel`.
//!
//! What lives here is the part that is genuinely this crate's: which of **its**
//! blocks a body can occupy, and the rule library's own travel-axis convention.
//! Everything else re-exports, so a program authored outside this repo still
//! reaches the walk by the name it has always had.
//!
//! `tests/support/mod.rs` delegates here. `tests/staging.rs` still carries its
//! own copy — that file's own header records why — and folding it in is a
//! follow-up, not a silence.
//!
//! # What the model is, and is not
//!
//! One walker: one cell horizontally at a time, stepping at most one cell up or
//! down, and — under [`reachable_with_fall`] only — walking off a ledge and
//! landing on the first floor below. Every step is decided by the engine's one
//! step rule, `delvewright_dsl::metrics::step_allowed`, which is also what
//! `delvec` routes with: a full-block rise is a jump, and a jump needs the cell
//! the head sweeps through clear.
//!
//! [`reachable_with_fall`] remains deliberately more permissive than the plain
//! walk, and its own doc records which direction of claim may use it.

use std::collections::BTreeSet;

use delvewright_schem::nav::Voxels;

use crate::model::VoxelModel;

pub use delvewright_schem::nav::{components, connected, reachable_from};

/// Cells a body and a sightline pass through.
///
/// Everything the rule library places is a full block except air and a floor
/// skull, which is neither a barrier nor an occluder — and which sits on the
/// exact cell an anchor names, so a naive "not air means solid" predicate would
/// report that niche unreachable. Outside the region counts as blocking: a body
/// that has left the model has left the thing being proved.
///
/// This is the crate's answer to [`Voxels::passable`], and it is the only part
/// of the walk this crate owns: it is a fact about the rule library's block
/// vocabulary, not about walking.
impl Voxels for VoxelModel {
    fn origin(&self) -> [i32; 3] {
        self.region().origin
    }

    fn size(&self) -> [i32; 3] {
        let s = self.region().size;
        [s[0] as i32, s[1] as i32, s[2] as i32]
    }

    fn passable(&self, pos: [i32; 3]) -> bool {
        match self.get(pos) {
            None => false,
            Some(block) => block.is_air() || block.name.ends_with("_skull"),
        }
    }
}

/// Cells a body and a sightline pass through — see the [`Voxels`] impl above.
pub fn passable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    Voxels::passable(model, pos)
}

/// A full block: what a floor is made of, and what stops an eye.
pub fn solid(model: &VoxelModel, pos: [i32; 3]) -> bool {
    delvewright_schem::nav::solid(model, pos)
}

/// A cell a player can stand in: two blocks of clearance over a full floor.
pub fn standable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    delvewright_schem::nav::standable(model, pos)
}

/// Every standable cell of the model.
pub fn standable_cells(model: &VoxelModel) -> BTreeSet<[i32; 3]> {
    delvewright_schem::nav::standable_cells(model)
}

/// [`connected`]'s ±1-step walk, plus a one-way **fall** — see
/// [`delvewright_schem::nav::reachable_with_fall`].
pub fn reachable_with_fall(
    model: &VoxelModel,
    cells: &BTreeSet<[i32; 3]>,
    from: &BTreeSet<[i32; 3]>,
    to: &BTreeSet<[i32; 3]>,
) -> bool {
    delvewright_schem::nav::reachable_with_fall(model, cells, from, to)
}

/// Where a body walks in: the standable cells on the region's four **vertical**
/// boundary faces, at grade. See [`delvewright_schem::nav::ground_entry`].
pub fn ground_entry(model: &VoxelModel) -> BTreeSet<[i32; 3]> {
    delvewright_schem::nav::ground_entry(model)
}

/// Is anything solid over this cell, inside the region?
/// See [`delvewright_schem::nav::sheltered`].
pub fn sheltered(model: &VoxelModel, pos: [i32; 3]) -> bool {
    delvewright_schem::nav::sheltered(model, pos)
}

/// The standable cells at each end of the model's local travel axis: the entry
/// (world `Z`-max, where the player comes in) and the exit (`Z`-min).
///
/// The convention is the rule library's own frame (`docs/reference/grammar.md`
/// §5b): local `Z`-max is the approach end and travel runs toward `Z`-min. It
/// stays in this crate because it is that convention and nothing else — a
/// structure template read off disk has no travel axis.
pub fn ends(model: &VoxelModel) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let region = model.region();
    let far = region.origin[2] + region.size[2] as i32 - 1;
    let near = region.origin[2];
    let cells = standable_cells(model);
    let entry = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit = cells.iter().copied().filter(|c| c[2] == near).collect();
    (entry, exit)
}
