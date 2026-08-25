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
//! blocks a body can occupy and which of them it can stand on, plus the rule
//! library's own travel-axis convention. Everything else re-exports, so a
//! program authored outside this repo still reaches the walk by the name it has
//! always had.
//!
//! `tests/support/mod.rs` delegates here. `tests/staging.rs` still carries its
//! own copy — that file's own header records why — and folding it in is a
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

use std::collections::BTreeSet;

use delvewright_schem::fluid::is_fluid;
use delvewright_schem::nav::Voxels;

use crate::model::VoxelModel;

pub use delvewright_schem::nav::{components, connected, reachable_from};

/// What a body may occupy, and what it may stand on, in this library's own
/// block vocabulary.
///
/// Everything the rule library places is a full block except air and a floor
/// skull, which is neither a barrier nor an occluder — and which sits on the
/// exact cell an anchor names, so a naive "not air means solid" predicate would
/// report that niche unreachable. Outside the region counts as blocking: a body
/// that has left the model has left the thing being proved.
///
/// These two methods are the whole of the walk this crate owns, and they are
/// facts about the rule library's block vocabulary rather than about walking —
/// which is why they are a pair and not one predicate. Occupancy and support
/// are separate questions, and water is the block that answers **no** to both.
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

    /// A body stands on stone, and it does not stand on the sea.
    ///
    /// The default reads "a floor is anything not passable", which is right for
    /// a vocabulary of full blocks and air and wrong for the one thing this
    /// library places that a body sinks through. Water is neither passable nor
    /// floor — the same shape the trait's own doc gives lava — so it is refused
    /// here rather than left to the default, and the walk stops proving a route
    /// over an open surface (spec-0038: a route never credits water).
    ///
    /// The fluids are [`delvewright_schem::fluid`]'s, not a second list: that
    /// module is where this workspace already keeps what a fluid is, measured
    /// on the pinned server, and a third fluid would be a pin change rather
    /// than an edit here. **`waterlogged` is deliberately not read.** A
    /// waterlogged stair is a stair — a block with a collision box, holding its
    /// own water and spreading none — and a body stands on it.
    ///
    /// Refusing only. A cell of water stays impassable, so this cannot admit a
    /// route the walk did not already have; it can only withdraw one. Whether a
    /// body may *wade* — occupy a shallow flooded cell standing on the floor
    /// beneath it — is a separate claim in the opposite direction, and it is
    /// not made here.
    fn floor(&self, pos: [i32; 3]) -> bool {
        match self.get(pos) {
            None => false,
            Some(block) => !is_fluid(&block.name) && !self.passable(pos),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockState;
    use crate::geom::Box3;

    /// A basin: stone at `y=0`, the block under test filling `y=1`, air above.
    /// The cell a body would stand in is `[1, 2, 1]`.
    fn basin(under_foot: BlockState) -> VoxelModel {
        let mut m = VoxelModel::new(Box3::at_origin([3, 5, 3]));
        let stone = BlockState::simple("minecraft:stone");
        for x in 0..3 {
            for z in 0..3 {
                m.set([x, 0, z], &stone).unwrap();
                m.set([x, 1, z], &under_foot).unwrap();
            }
        }
        m
    }

    /// The rule, and the one the walk got wrong: a body stands on the stone and
    /// it does not stand on the sea. Both fluids, and a flow as well as a
    /// source — vanilla derives `level` on its own clock, so a walk that
    /// refused only `level=0` would credit every cell the tide is still moving
    /// through.
    #[test]
    fn a_body_stands_on_stone_and_not_on_water() {
        assert!(standable(
            &basin(BlockState::simple("minecraft:stone")),
            [1, 2, 1]
        ));
        for fluid in [
            BlockState::with("minecraft:water", [("level", "0")]),
            BlockState::with("minecraft:water", [("level", "3")]),
            BlockState::simple("minecraft:lava"),
        ] {
            let m = basin(fluid.clone());
            assert!(
                !standable(&m, [1, 2, 1]),
                "a body was credited standing on {}",
                fluid.name
            );
            assert!(
                standable_cells(&m).is_empty(),
                "{} left a standable cell somewhere",
                fluid.name
            );
        }
    }

    /// A waterlogged block is a block. It has a collision box, it holds its own
    /// water and spreads none (`delvewright_schem::fluid`), and a body stands
    /// on it — so the rule keys on the block id and deliberately never on the
    /// `waterlogged` property.
    #[test]
    fn a_waterlogged_block_is_still_a_floor() {
        let stair = BlockState::with(
            "minecraft:stone_brick_stairs",
            [
                ("facing", "north"),
                ("half", "bottom"),
                ("waterlogged", "true"),
            ],
        );
        assert!(standable(&basin(stair), [1, 2, 1]));
    }

    /// The change is one-directional, and this is what pins it: water stays
    /// **impassable**, so a cell of water is still somewhere no body may be.
    /// Nothing here can therefore admit a route the walk did not already have
    /// — it can only withdraw one. The same pair of answers `delvec`'s own
    /// routing model gives a flooded cell: impassable, and not floor.
    ///
    /// [`solid`] is deliberately still **true** over water, and that is not an
    /// oversight. It answers "what stops an eye", which is a sightline
    /// question, not a standing one — the two were the same set while water
    /// was a full block and are not any more. Anything that wants "can a body
    /// stand on this" must ask [`Voxels::floor`], which is what
    /// [`standable`] does.
    #[test]
    fn water_is_never_occupied_and_never_a_floor() {
        let m = basin(BlockState::with("minecraft:water", [("level", "0")]));
        assert!(!passable(&m, [1, 1, 1]), "a body may not be in the water");
        assert!(
            !Voxels::floor(&m, [1, 1, 1]),
            "and it may not stand on it either"
        );
        assert!(
            solid(&m, [1, 1, 1]),
            "water still stops an eye — `solid` is the sightline question, and this \
             change did not touch it"
        );
        // Wading is a claim in the opposite direction and is deliberately not
        // made here: the cell of water itself is refused, not admitted.
        assert!(!standable(&m, [1, 1, 1]));
    }

    /// The shape the campaign actually ships, and the one a synthetic room
    /// misses: a flood with a body-height air pocket over it. Every cell of the
    /// surface used to answer `standable`, so a walk crossed the ward; the dry
    /// spine beside it is what a body really has.
    #[test]
    fn an_open_flood_is_not_a_floor_and_the_spine_beside_it_is() {
        let mut m = VoxelModel::new(Box3::at_origin([5, 5, 5]));
        let stone = BlockState::simple("minecraft:stone");
        let water = BlockState::with("minecraft:water", [("level", "0")]);
        for x in 0..5 {
            for z in 0..5 {
                m.set([x, 0, z], &stone).unwrap();
                // A one-cell raised spine down the middle, flooded either side.
                if x == 2 {
                    m.set([x, 1, z], &stone).unwrap();
                } else {
                    m.set([x, 1, z], &water).unwrap();
                }
            }
        }
        let cells = standable_cells(&m);
        assert_eq!(
            cells.len(),
            5,
            "only the spine is standable, and it is 5 cells long: {cells:?}"
        );
        assert!(cells.iter().all(|c| c[0] == 2 && c[1] == 2));
    }
}
