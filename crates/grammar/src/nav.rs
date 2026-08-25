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

use delvewright_dsl::blockshape::Collision;
use delvewright_schem::nav::Voxels;

use crate::model::VoxelModel;

pub use delvewright_schem::nav::{components, connected, reachable_from};

/// What a body may occupy, what it may stand on, and how high that floor is —
/// **asked, not decided, here**.
///
/// The three answers come from [`delvewright_dsl::blockshape`] (spec-0056), the
/// one place in this workspace that knows what a vanilla block state does to a
/// body. This impl used to answer *air, or a block whose name ends in `_skull`*
/// and call everything else a full solid cube, which meant no grammar-built zone
/// could hold a torch, a candle, a carpet, a pressure plate or a tuft of grass
/// anywhere a player was meant to walk: the decoration severed the room, and
/// three contract gates went red over one bed of glow lichen. Meanwhile `delvec`
/// held a real collision table and could not lend it here.
///
/// What is still genuinely this crate's is one convention, and it is content
/// rather than mechanism: **a floor skull is passable.** The rule library places
/// one on the exact cell an anchor names, so a walk that read its collision box
/// (8/16, a partial floor) would report that niche unreachable. That is a fact
/// about this library's vocabulary, not about the game, so it is written here and
/// nowhere else.
///
/// Outside the region counts as blocking: a body that has left the model has left
/// the thing being proved.
impl Voxels for VoxelModel {
    fn origin(&self) -> [i32; 3] {
        self.region().origin
    }

    fn size(&self) -> [i32; 3] {
        let s = self.region().size;
        [s[0] as i32, s[1] as i32, s[2] as i32]
    }

    fn passable(&self, pos: [i32; 3]) -> bool {
        match self.collision(pos) {
            None => false,
            Some(_) if is_floor_skull(self, pos) => true,
            Some(class) => class.passes_body(),
        }
    }

    /// A body stands on stone; it does not stand on the sea, on a torch, or on
    /// the top of a fence.
    ///
    /// Not the complement of [`Voxels::passable`], which is why the trait asks
    /// twice. Three classes answer **no** to both questions, and they are why the
    /// default reading (`!passable`) is wrong for this vocabulary: a fluid
    /// (spec-0038 — a route never credits water, and nothing stands on a
    /// surface), a tall barrier (1.5 blocks on a 1-block cell, above the jump
    /// apex), and every thin decoration (a body walks through it and rests on
    /// whatever is below).
    ///
    /// **`waterlogged` is deliberately not read.** A waterlogged stair is a stair
    /// — a block with a collision box, holding its own water and spreading none —
    /// and a body stands on it.
    fn floor(&self, pos: [i32; 3]) -> bool {
        match self.collision(pos) {
            None => false,
            Some(_) if is_floor_skull(self, pos) => false,
            Some(class) => class.supports_body(),
        }
    }

    /// The measured top face of the block a body rests on, in sixteenths.
    ///
    /// The default answers a full cube for anything that is a floor at all, which
    /// over-states every rise and therefore only ever refuses a step vanilla
    /// admits. This vocabulary has partial-height blocks in it — slabs, snow
    /// drifts, `dirt_path` — so it answers with the measurement instead, and a
    /// body walks up onto a bottom slab without being asked for jump headroom.
    fn floor_top_16(&self, support: [i32; 3]) -> i64 {
        match self.collision(support).and_then(Collision::floor_top_16) {
            Some(top) => i64::from(top),
            // Not a floor at all. The walk reads this only for a cell `floor`
            // has already accepted, so it is unreachable in practice; a full cube
            // is the refusing answer if it ever is reached.
            None => delvewright_dsl::metrics::FULL_16,
        }
    }
}

/// The rule library's own convention: a skull laid on a floor cell is a marker,
/// not an obstacle.
///
/// Vanilla gives a skull an 8/16 collision box, so the shared table calls it a
/// partial floor — correct for the game and wrong for this library, which puts
/// one on the exact cell an anchor names and needs a body able to stand there.
/// Content, therefore local, therefore stated once.
fn is_floor_skull(model: &VoxelModel, pos: [i32; 3]) -> bool {
    model.get(pos).is_some_and(|b| b.name.ends_with("_skull"))
}

/// Cells a body and a sightline pass through — see the [`Voxels`] impl above.
pub fn passable(model: &VoxelModel, pos: [i32; 3]) -> bool {
    Voxels::passable(model, pos)
}

/// **What stops an eye**: anything in the box a body cannot pass through.
///
/// This used to be the same set as "what a floor is made of" and is not any
/// more: water stops an eye and is not a floor. So a caller asking "can a body
/// stand on this cell" wants [`Voxels::floor`] — which is what [`standable`]
/// asks — and one asking about a sightline, an occluder or a landing wants
/// this. The two are named apart deliberately; the campaign that made the
/// difference is a drowned citadel, where the whole ward is one and not the
/// other.
pub fn solid(model: &VoxelModel, pos: [i32; 3]) -> bool {
    delvewright_schem::nav::solid(model, pos)
}

/// A cell a player can stand in: two cells of clearance over a floor — a floor
/// being what [`Voxels::floor`] says it is, which is not the complement of
/// passable and is never a fluid.
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
