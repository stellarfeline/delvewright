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
///
/// **It is the convention's accessor and it is not a question about a piece.**
/// A caller holding a model it authored to the §5b frame, at a box it chose, is
/// entitled to ask where that frame's two ends are; the zone tests are exactly
/// that caller. Nothing that JUDGES an arbitrary piece may ask it, because the
/// two premises it rests on are the caller's and not the piece's: that world `Z`
/// is the travel axis at all, and that the piece runs the whole length of its
/// box. Every §5b rule opens with `z(Largest)` and turns its length onto the
/// longer horizontal axis of whatever box it is handed, so even a library piece
/// breaks the first premise the moment its box is wider than it is deep. The
/// question a gate has to ask is [`open_sides`], which asks the piece.
pub fn ends(model: &VoxelModel) -> (BTreeSet<[i32; 3]>, BTreeSet<[i32; 3]>) {
    let region = model.region();
    let far = region.origin[2] + region.size[2] as i32 - 1;
    let near = region.origin[2];
    let cells = standable_cells(model);
    let entry = cells.iter().copied().filter(|c| c[2] == far).collect();
    let exit = cells.iter().copied().filter(|c| c[2] == near).collect();
    (entry, exit)
}

/// **Which of the piece's vertical sides its standable floor reaches, and
/// where** — the unit direction of each open side, paired with the cells on it.
///
/// The question `traversable` has to ask of a piece that declares no spatial
/// contract: *which faces does this piece open on?* A contract answers it for
/// all six directions by naming doors ([`crate::contract::exterior_faces`]),
/// which is the general mechanism and stays the authority wherever a piece has
/// one. With no contract there is nothing declared to read, so the sides are
/// derived from the blocks: a side is open where the standable floor reaches
/// its plane.
///
/// Derived, and never assumed. The rule this replaced took the world `Z`-max
/// and `Z`-min planes and nothing else, so it asked about the north and south
/// faces of every piece in the corpus — a straight east–west corridor has no
/// standable cell on either, and the gate over it examined zero objects while
/// the piece it was judging walked end to end perfectly well.
///
/// **Four sides and not six, and it is the derivation that has four rather
/// than a rule imposed on it.** Neither horizontal plane of the region can hold
/// a standable cell in the first place, because outside the box blocks (see the
/// [`Voxels`] impl above): on the top plane the body's head is outside and the
/// cell is not passable, on the bottom plane its floor is outside and there is
/// nothing to stand on. So the four vertical sides are the whole of what a
/// derivation from standable cells could ever read, and this returns all of it.
/// [`ground_entry`] lands on the same four by the same arithmetic. A piece
/// entered from above is therefore not a piece this can read at all — it says
/// so by declaring the face, which the contract exports on any of the six
/// sides, and a `traversable` claim on one that declares nothing binds too low
/// and is refused rather than passed.
///
/// A side with nothing standable on it is **absent**, not empty: it is not a way
/// out, and a caller counting the returned sides would otherwise get four on
/// every piece, which is a constant wearing a binding count's clothes. An axis
/// one cell thick yields **one** side rather than two, because its two planes
/// are the same plane and a body cannot walk from a face to itself.
///
/// Ordered by direction, so a pair enumeration over the result is total
/// (ADR-0006).
pub fn open_sides(model: &VoxelModel) -> Vec<([i32; 3], BTreeSet<[i32; 3]>)> {
    let region = model.region();
    let min = region.origin;
    let cells = standable_cells(model);
    let mut out = Vec::new();
    // Sorted by direction here rather than afterwards: `[-1,0,0] < [0,0,-1] <
    // [0,0,1] < [1,0,0]` is the order `contract::FaceDir` derives, and the two
    // enumerations naming their sides the same way is what lets one walk judge
    // declared faces and derived sides without knowing which it was handed.
    for (axis, dir) in [
        (0usize, [-1, 0, 0]),
        (2usize, [0, 0, -1]),
        (2usize, [0, 0, 1]),
        (0usize, [1, 0, 0]),
    ] {
        let thickness = region.size[axis];
        if dir[axis] > 0 && thickness <= 1 {
            continue;
        }
        let plane = min[axis]
            + if dir[axis] > 0 {
                thickness as i32 - 1
            } else {
                0
            };
        let on: BTreeSet<[i32; 3]> = cells.iter().copied().filter(|c| c[axis] == plane).collect();
        if !on.is_empty() {
            out.push((dir, on));
        }
    }
    out
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

    /// **The owner's room, and the whole reason spec-0056 exists.**
    ///
    /// A room of ordinary height — floor, two courses, ceiling — with the things
    /// anybody puts in one: a torch on the floor, a wall torch on the wall, a
    /// candle, a carpet runner and a pressure plate, laid in one course across
    /// the middle. Under the old rule (*air, or a name ending in `_skull`*) every
    /// one of them was a full solid cube: the decorated cell could not be
    /// occupied, and the cell above it lost its headroom to the ceiling, so a
    /// whole course of floor vanished and the room was severed in two.
    ///
    /// This is a **set** assertion, not a count, and that is deliberate: a wrong
    /// collision table moves which cells are standable, and it cannot leave the
    /// set byte-identical to the bare room's.
    #[test]
    fn a_decorated_room_is_the_same_room() {
        /// A 9x4x9 stone box, hollow, with a doorway at grade in the `z=0` wall.
        fn room() -> VoxelModel {
            let mut m = VoxelModel::new(Box3::at_origin([9, 4, 9]));
            let stone = BlockState::simple("minecraft:stone_bricks");
            for x in 0..9 {
                for y in 0..4 {
                    for z in 0..9 {
                        let shell = y == 0 || y == 3 || x == 0 || x == 8 || z == 0 || z == 8;
                        if shell {
                            m.set([x, y, z], &stone).unwrap();
                        }
                    }
                }
            }
            // the doorway
            m.set([4, 1, 0], &BlockState::air()).unwrap();
            m.set([4, 2, 0], &BlockState::air()).unwrap();
            m
        }

        let bare = room();
        let bare_cells = standable_cells(&bare);
        let entry = ground_entry(&bare);
        assert_eq!(
            entry,
            BTreeSet::from([[4, 1, 0]]),
            "the doorway is the way in"
        );
        assert_eq!(
            reachable_from(&bare, &bare_cells, &entry),
            bare_cells,
            "an empty room is walkable end to end"
        );

        // Now dress it: one course across the room, wall to wall.
        let mut lit = room();
        let decor = [
            BlockState::simple("minecraft:torch"),
            BlockState::with("minecraft:wall_torch", [("facing", "north")]),
            BlockState::with(
                "minecraft:white_candle",
                [("candles", "3"), ("lit", "true"), ("waterlogged", "false")],
            ),
            BlockState::simple("minecraft:red_carpet"),
            BlockState::with("minecraft:stone_pressure_plate", [("powered", "false")]),
            BlockState::simple("minecraft:short_grass"),
            BlockState::simple("minecraft:glow_lichen"),
        ];
        for (i, x) in (1..8).enumerate() {
            lit.set([x, 1, 4], &decor[i % decor.len()]).unwrap();
        }

        let lit_cells = standable_cells(&lit);
        assert_eq!(
            lit_cells, bare_cells,
            "decorating the floor moved a standable cell; the walk is reading a \
             torch as a wall again"
        );
        assert_eq!(
            reachable_from(&lit, &lit_cells, &entry),
            bare_cells,
            "the decoration severed the room"
        );
        // Binding, computed from the objects rather than written down beside
        // them: 7 x 7 interior floor + the doorway, on one course.
        assert_eq!(bare_cells.len(), 7 * 7 + 1);
        assert_eq!(
            lit_cells.iter().filter(|c| c[2] == 4).count(),
            7,
            "the decorated course itself is where a body stands"
        );
    }

    /// **The step a body takes onto a bottom slab is a walk, not a jump.**
    ///
    /// The walk used to read every floor as a full cube, which turns an 8/16
    /// auto-step into a 16/16 jump — and a jump is the one step that demands the
    /// cell the head sweeps through be clear. So a beam over the low side
    /// refused a step vanilla walks. The error only ever ran in the refusing
    /// direction, so this is a step the engine gained rather than one it stopped
    /// losing.
    #[test]
    fn a_body_walks_up_onto_a_bottom_slab_under_a_beam_that_would_stop_a_jump() {
        let mut m = VoxelModel::new(Box3::at_origin([3, 5, 1]));
        let stone = BlockState::simple("minecraft:stone_bricks");
        let slab = BlockState::with("minecraft:stone_brick_slab", [("type", "bottom")]);
        for x in 0..3 {
            m.set([x, 0, 0], &stone).unwrap();
            m.set([x, 4, 0], &stone).unwrap(); // ceiling
        }
        m.set([1, 1, 0], &slab).unwrap();
        m.set([2, 1, 0], &slab).unwrap();
        // The cell a jumping body's head would sweep through, blocked.
        m.set([0, 3, 0], &stone).unwrap();

        assert_eq!(
            Voxels::floor_top_16(&m, [1, 1, 0]),
            8,
            "a bottom slab is measured, not read as a cube"
        );
        let cells = standable_cells(&m);
        assert_eq!(
            cells,
            BTreeSet::from([[0, 1, 0], [1, 2, 0], [2, 2, 0]]),
            "on the stone, and on top of each slab"
        );
        assert!(
            delvewright_schem::nav::connected(
                &m,
                &cells,
                &BTreeSet::from([[0, 1, 0]]),
                &BTreeSet::from([[2, 2, 0]])
            ),
            "half a block up is an auto-step and asks nothing of the beam"
        );
        // And the beam is real: raise the same step to a full block and the walk
        // refuses it, so the green above is the measurement and not a hole.
        let mut full = VoxelModel::new(Box3::at_origin([3, 5, 1]));
        for x in 0..3 {
            full.set([x, 0, 0], &stone).unwrap();
            full.set([x, 4, 0], &stone).unwrap();
        }
        full.set([1, 1, 0], &stone).unwrap();
        full.set([2, 1, 0], &stone).unwrap();
        full.set([0, 3, 0], &stone).unwrap();
        let full_cells = standable_cells(&full);
        assert!(
            !delvewright_schem::nav::connected(
                &full,
                &full_cells,
                &BTreeSet::from([[0, 1, 0]]),
                &BTreeSet::from([[2, 2, 0]])
            ),
            "a whole block up is a jump, and the beam is in the way"
        );
    }

    /// The other side of the same table: what is still a wall.
    ///
    /// A change that only ever adds passable cells can break a seal, so the
    /// classes that must keep sealing are asserted here rather than assumed. A
    /// fence and a wall are 1.5 blocks tall on a 1-block cell — a body neither
    /// walks through one nor stands on top of one — and everything the table does
    /// not recognise is a full cube.
    #[test]
    fn a_fence_is_still_a_wall_and_nothing_stands_on_it() {
        for barrier in ["minecraft:oak_fence", "minecraft:cobblestone_wall"] {
            let m = basin(BlockState::simple(barrier));
            assert!(!passable(&m, [1, 1, 1]), "{barrier} is not walked through");
            assert!(
                !Voxels::floor(&m, [1, 1, 1]),
                "{barrier} is 1.5 tall: no walking body stands on its top"
            );
            assert!(standable_cells(&m).is_empty(), "{barrier}");
        }
        // And a closed door stays a door: unrecognised means full cube, which
        // can only refuse a route, never invent one.
        let door = BlockState::with(
            "minecraft:oak_door",
            [("half", "lower"), ("facing", "north"), ("open", "false")],
        );
        assert!(!passable(&basin(door), [1, 1, 1]));
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
