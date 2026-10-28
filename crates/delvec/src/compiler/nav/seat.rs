//! **Where a respawn puts a body** — the one seating rule for a `set-checkpoint`
//! and a `bonfire` alike.
//!
//! A checkpoint names two things, and they are not one cell: the **rest
//! object** — the anchor, where a bonfire's hitbox stands and where a prefab may
//! lay the campfire the party rests at — and the **seat**, the cell a dead
//! body is put back on. The seat is derived here and never written: the anchor
//! cell itself when a body may be put down there, else the safe standing cell
//! nearest the rest object, on the side the party walks up from.
//!
//! Every reader of "where does a respawn put the party" reads [`World::rest_seat_of`]
//! (or, for the emitter, which holds no world, [`Plan::seat`], fixed from the
//! same derivation): the no-stranding and placement proofs (`DW0315`/`DW0316`),
//! the respawn safe zone (`DW0478`), the lethal-seat proof (`DW0511`), the stake
//! table and the death plan the bot checks its respawn against, the emitted
//! `spawnpoint`, `dw:cp` mirror and re-seat teleport.
//!
//! What "may be put down" means ([`World::is_safe_seat`]): a standing cell in
//! the model's own sense ([`World::is_standable`]: floor under it, room for the
//! body, no killing volume or furniture), whose body meets no block that hurts
//! it ([`delvewright_dsl::blockshape::hurts_body`]) — not the floor it stands
//! on, not its feet cell, not its head cell. A lit campfire hurts every living
//! body whose box enters its cell, and its collision top is 7/16 of a block, so
//! a body standing on it has its feet inside it
//! (`docs/reference/campfire-and-spawnpoint.md`). The table is read without the
//! block's state: an unlit campfire is seated beside too, the direction that can
//! only move a seat, never put one in a fire.

use super::*;
use crate::compiler::cellset::CellSet;
use crate::compiler::plan::{CheckpointPlan, Plan};

/// How a candidate seat ranks within its ring: the length of the party's walk
/// to it, its squared distance to the rest object, the cell.
type SeatKey = (usize, i32, [i32; 3]);

/// What a [`World`] knows about putting a body down: the cells whose block
/// hurts a body, and each checkpoint's seat once derived
/// ([`World::with_rest_seats`]).
#[derive(Clone, Debug, Default)]
pub struct Seating {
    /// Cells holding a block that hurts a body that meets it.
    pub(in crate::compiler::nav) hurting: CellSet,
    /// Each checkpoint's seat, by [`CheckpointPlan::index`]; `None` for one no
    /// safe cell stands beside (`DW0316`). Empty until derived.
    seats: std::sync::Arc<Vec<Option<[i32; 3]>>>,
    /// Whether [`World::with_rest_seats`] has run on this world.
    derived: bool,
}

impl Seating {
    pub(in crate::compiler::nav) fn of_hurting(hurting: CellSet) -> Seating {
        Seating {
            hurting,
            ..Seating::default()
        }
    }
}

impl World {
    /// **Whether a body may be put down on `c` by declaration** — a respawn
    /// seat. Standable ([`World::is_standable`]) and meeting no block that
    /// hurts: the support under it (a campfire's top or magma underfoot), the
    /// feet cell, or the head cell.
    pub fn is_safe_seat(&self, c: [i32; 3]) -> bool {
        self.standable(c) && !self.seat_hurts(c)
    }

    /// Whether a body standing on `c` meets a block that hurts it.
    fn seat_hurts(&self, c: [i32; 3]) -> bool {
        (-1..=1).any(|dy| self.seating.hurting.contains(&[c[0], c[1] + dy, c[2]]))
    }

    /// **The seat rule**: where a respawn at the rest object `rest` puts a body,
    /// for a party that walks up from `from`.
    ///
    /// `rest` itself when [`World::is_safe_seat`] holds there. Otherwise the
    /// safe cells around it, nearest ring first (Chebyshev distance 1, then 2,
    /// up to [`SNAP_RADIUS`]); within a ring, one a body from `from` reaches
    /// before one it does not — the reach graph's answer
    /// ([`World::body_reach`], spec-0113), the one answer to where a body can
    /// get to — then the shorter walk to it ([`World::find_path`]'s length; a
    /// cell only a launch reaches after every cell a walk does), then the
    /// nearest to `rest`, then the least cell. `None` when no safe cell lies
    /// within the radius.
    pub fn rest_seat(&self, rest: [i32; 3], from: Option<[i32; 3]>) -> Option<[i32; 3]> {
        if self.is_safe_seat(rest) {
            return Some(rest);
        }
        let origin = from.and_then(|f| self.snap_standable(f, SNAP_RADIUS));
        let reached: std::collections::BTreeSet<[i32; 3]> = origin
            .map(|o| self.body_reach(&[o]).standing())
            .unwrap_or_default();
        for ring in 1..=SNAP_RADIUS {
            // `(walk, distance², cell)`: the walk from the approach first.
            let mut best: Option<(SeatKey, [i32; 3])> = None;
            for dy in -ring..=ring {
                for dz in -ring..=ring {
                    for dx in -ring..=ring {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != ring {
                            continue;
                        }
                        let c = [rest[0] + dx, rest[1] + dy, rest[2] + dz];
                        if !self.is_safe_seat(c) {
                            continue;
                        }
                        let walk = if reached.contains(&c) {
                            origin
                                .and_then(|o| self.find_path(o, c))
                                .map_or(usize::MAX - 1, |p| p.len())
                        } else {
                            usize::MAX
                        };
                        let key = (walk, dx * dx + dy * dy + dz * dz, c);
                        if best.is_none_or(|(k, _)| key < k) {
                            best = Some((key, c));
                        }
                    }
                }
            }
            if let Some((_, c)) = best {
                return Some(c);
            }
        }
        None
    }

    /// **The world a seat is derived and judged over: the world as shipped**
    /// — this world with the blocks every pressable lays at load standing in
    /// it ([`Plan::load_prop_events`], the list `setup_finish` writes and the
    /// shipped world lays), so a campfire a bonfire's `prop` lays on its own
    /// cell hurts as one a piece placed, and a `prop` laid over a piece's
    /// block replaces it. No beat's write is in it: a respawn bundle runs
    /// after the body is seated, and a later beat's write is judged by the
    /// proofs that walk from the seat. The derivation
    /// ([`World::with_rest_seats`]) and the placement proof (`DW0316`) both
    /// read it, so a seat is never derived over one world and judged over
    /// another. `None` when no pressable lays a block, so the caller keeps this
    /// world.
    pub(in crate::compiler::nav) fn seating_world(&self, plan: &Plan) -> Option<World> {
        let props: crate::compiler::plan::RegionEvents = plan.load_prop_events().into();
        if props.is_empty() {
            return None;
        }
        let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
        let st = self.region_state_at(&props, 0, &ancestor);
        Some(self.with_region_state(&st))
    }

    /// This world with every checkpoint's seat derived over it
    /// ([`World::rest_seat`], from [`approach_origin`]). Every world a proof
    /// about a respawn walks takes its seats here: the build's final world,
    /// once the plan its proofs read is settled (`emit::build`, after links are
    /// taken), and each edit batch's (`edit::replay`).
    ///
    /// Each seat is derived over the world as shipped
    /// ([`World::seating_world`]).
    pub fn with_rest_seats(mut self, plan: &Plan) -> World {
        let shipped = self.seating_world(plan);
        let seating = shipped.as_ref().unwrap_or(&self);
        let seats: Vec<Option<[i32; 3]>> = plan
            .checkpoints
            .iter()
            .map(|cp| seating.rest_seat(cp.at, approach_origin(plan, cp)))
            .collect();
        self.seating.seats = std::sync::Arc::new(seats);
        self.seating.derived = true;
        self
    }

    /// **The seat of checkpoint `cp`** on this world: `None` for one no safe
    /// cell stands beside (`DW0316`'s finding).
    ///
    /// # Panics
    ///
    /// When the seats were never derived on this world
    /// ([`World::with_rest_seats`]): a proof reading a seat nobody derived
    /// would judge a cell no build emits, which is a compiler defect.
    pub fn rest_seat_of(&self, cp: &CheckpointPlan) -> Option<[i32; 3]> {
        assert!(
            self.seating.derived,
            "the respawn seats were read on a world that never derived them \
             (`World::with_rest_seats`) — a compiler defect"
        );
        self.seating.seats.get(cp.index).copied().flatten()
    }

    /// Every derived seat, by checkpoint index — what [`Plan::fix_seats`]
    /// hands the emitter.
    pub fn rest_seats(&self) -> Vec<Option<[i32; 3]>> {
        self.seating.seats.as_ref().clone()
    }
}

/// **Where the party walks up to checkpoint `cp` from**: where it stands when
/// the beat that sets or arms it fires — the last critical-path position at or
/// before its firing step, the entry spawn for a checkpoint set before any
/// beat. `None` for a campaign with no position at all.
pub fn approach_origin(plan: &Plan, cp: &CheckpointPlan) -> Option<[i32; 3]> {
    critical_positions(plan)
        .iter()
        .rfind(|p| p.src_step <= cp.fire_step)
        .map(|p| p.pos)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::nav::testkit::*;

    /// A 5 × 5 floor at y 64 (stand at y 65), a campfire on `[2, 65, 2]`.
    fn hearth(fire: &str) -> World {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..5 {
            for z in 0..5 {
                cells.push(([x, 64, z], "minecraft:stone"));
            }
        }
        cells.push(([2, 65, 2], fire));
        blocks_world(&cells)
    }

    /// The old shape: the seat on the rest object's own cell. A campfire's cell
    /// is not a seat — nor is the cell over it, whose body has its feet in the
    /// fire — and the derived seat is the floor beside it.
    #[test]
    fn a_campfire_rest_object_seats_the_body_beside_it() {
        for fire in [
            "minecraft:campfire[lit=true]",
            "minecraft:campfire[lit=false]",
        ] {
            let w = hearth(fire);
            assert!(!w.is_safe_seat([2, 65, 2]), "{fire}: its own cell");
            assert!(!w.is_safe_seat([2, 66, 2]), "{fire}: on top of it");
            let seat = w
                .rest_seat([2, 65, 2], None)
                .expect("floor beside the fire");
            assert_ne!(seat, [2, 65, 2], "{fire}");
            assert!(w.is_safe_seat(seat), "{fire}: {seat:?}");
            let d = (seat[0] - 2).abs().max((seat[2] - 2).abs());
            assert_eq!(d, 1, "{fire}: the seat is next to the rest object");
        }
    }

    /// The seat is on the side the party walks up from: from the west, the
    /// west cell; from the east, the east cell.
    #[test]
    fn the_seat_is_on_the_side_the_party_approaches_from() {
        let w = hearth("minecraft:campfire[lit=true]");
        assert_eq!(w.rest_seat([2, 65, 2], Some([0, 65, 2])), Some([1, 65, 2]));
        assert_eq!(w.rest_seat([2, 65, 2], Some([4, 65, 2])), Some([3, 65, 2]));
        assert_eq!(w.rest_seat([2, 65, 2], Some([2, 65, 0])), Some([2, 65, 1]));
    }

    /// **A seat is ranked by the walk that reads leaves**: an iron trapdoor
    /// standing open in the cell west of the fire (`facing=east`, its panel
    /// against that cell's west face) stops the party's straight step from
    /// the west, so the seat is not that cell but one the walk reaches round
    /// the panel. With the trapdoor lifted the same approach seats on it.
    #[test]
    fn a_seat_is_ranked_by_the_walk_that_reads_a_leaf() {
        let fire = "minecraft:campfire[lit=true]";
        let hatch = "minecraft:iron_trapdoor[facing=east,half=bottom,open=true,powered=false,waterlogged=false]";
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..5 {
            for z in 0..5 {
                cells.push(([x, 64, z], "minecraft:stone"));
            }
        }
        cells.push(([2, 65, 2], fire));
        let lifted = blocks_world(&cells);
        assert!(!lifted.has_leaves());
        let from = Some([0, 65, 2]);
        assert_eq!(lifted.rest_seat([2, 65, 2], from), Some([1, 65, 2]));

        cells.push(([1, 65, 2], hatch));
        let w = blocks_world(&cells);
        assert_eq!(w.hatch_count(), 1);
        let straight = w.find_path([0, 65, 2], [1, 65, 2]).map(|p| p.len());
        assert_ne!(straight, Some(2), "the panel stops the straight step");
        let seat = w
            .rest_seat([2, 65, 2], from)
            .expect("floor beside the fire");
        assert_ne!(seat, [1, 65, 2], "the panel's side is not the nearest walk");
        assert!(w.is_safe_seat(seat), "{seat:?}");
        assert!(w.find_path([0, 65, 2], seat).is_some(), "{seat:?}");
    }

    /// **A seat is reached over the reach graph** (spec-0113 with the seat
    /// rule). The rest object stands on a campfire between two floor cells:
    /// the west one is shut in under a low roof no body gets into, and the
    /// east one is across a two-wide gap from where the party walks up — no
    /// walk reaches either. Ranked by the walk alone the two tie and the least
    /// cell, the shut-in west one, would seat a respawn where the party never
    /// stands; the reach graph's jump reaches the east one, and it is the
    /// seat. The perturbation only the graph answers: with the far platform
    /// one more cell away, past every jump, neither is reached and the tie
    /// falls back to the least cell.
    #[test]
    fn a_seat_is_reached_over_the_reach_graph() {
        let world = |far: i32| {
            let fire = "minecraft:campfire[lit=true]";
            let stone = "minecraft:stone";
            let mut cells: Vec<([i32; 3], &str)> = Vec::new();
            for x in -1..=far + 3 {
                for y in 63..=67 {
                    cells.push(([x, y, -1], stone));
                    cells.push(([x, y, 1], stone));
                }
            }
            for y in 64..=67 {
                cells.push(([-1, y, 0], stone));
                cells.push(([far + 4, y, 0], stone));
            }
            cells.push(([0, 64, 0], stone)); // the shut-in floor
            cells.push(([1, 64, 0], fire)); // the rest object's campfire
            cells.push(([2, 64, 0], stone)); // the floor across the gap
            for x in far..=far + 3 {
                cells.push(([x, 64, 0], stone)); // the approach
            }
            cells.push(([1, 66, 0], stone)); // no body passes over the fire
            for x in -1..=1 {
                cells.push(([x, 67, 0], stone)); // the low roof
            }
            for x in 3..far {
                cells.push(([x, 58, 0], stone)); // the pit's floor
            }
            blocks_world(&cells)
        };
        let rest = [1, 65, 0];
        let w = world(5);
        let from = [7, 65, 0];
        assert!(!w.is_safe_seat(rest));
        assert!(w.is_safe_seat([0, 65, 0]) && w.is_safe_seat([2, 65, 0]));
        assert!(
            w.find_path(from, [2, 65, 0]).is_none(),
            "no walk crosses the gap"
        );
        assert!(w.find_path(from, [0, 65, 0]).is_none());
        let reached = w.body_reach(&[from]).standing();
        assert!(reached.contains(&[2, 65, 0]), "a jump crosses the gap");
        assert!(
            !reached.contains(&[0, 65, 0]),
            "nothing gets under the roof"
        );
        assert_eq!(w.rest_seat(rest, Some(from)), Some([2, 65, 0]));

        let wide = world(9);
        let from = [11, 65, 0];
        assert!(!wide.body_reach(&[from]).standing().contains(&[2, 65, 0]));
        assert_eq!(wide.rest_seat(rest, Some(from)), Some([0, 65, 0]));
    }

    /// A safe anchor cell is its own seat: a plain checkpoint on open floor
    /// seats where it always did.
    #[test]
    fn a_safe_anchor_is_its_own_seat() {
        let w = hearth("minecraft:campfire[lit=true]");
        assert_eq!(w.rest_seat([0, 65, 0], Some([4, 65, 4])), Some([0, 65, 0]));
    }

    /// A block a region write lays is judged as a block a piece placed: a
    /// campfire laid at load (a pressable's `prop`) hurts, so neither its cell
    /// nor the cell over it is a seat; air laid over a placed campfire takes
    /// the fire away.
    #[test]
    fn a_laid_block_hurts_as_a_placed_one() {
        use crate::compiler::nav::route::region::RegionState;
        let c = [2, 65, 2];
        let laid = |block: Option<&str>| vec![((c, c), block.map(str::to_string))];
        let bare = hearth("minecraft:air");
        assert!(bare.is_safe_seat(c), "open floor before the write");
        let st = RegionState {
            solid: [c].into_iter().collect(),
            laid: laid(Some("minecraft:campfire[lit=true]")),
            ..RegionState::default()
        };
        let fired = bare.with_region_state(&st);
        assert!(!fired.is_safe_seat(c), "the laid fire's own cell");
        assert!(
            !fired.is_safe_seat([2, 66, 2]),
            "on top of the laid fire: its feet are in it"
        );
        let st = RegionState {
            cleared: [c].into_iter().collect(),
            laid: laid(None),
            ..RegionState::default()
        };
        let put_out = hearth("minecraft:campfire[lit=true]").with_region_state(&st);
        assert!(put_out.is_safe_seat(c), "the fire cleared by a write");
    }

    /// A rest object with no safe cell within the radius has no seat.
    #[test]
    fn a_rest_object_with_no_safe_cell_beside_it_has_no_seat() {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in -4..=4 {
            for z in -4..=4 {
                cells.push(([x, 64, z], "minecraft:stone"));
                cells.push(([x, 65, z], "minecraft:magma_block"));
            }
        }
        let w = blocks_world(&cells);
        assert_eq!(w.rest_seat([0, 65, 0], None), None);
    }
}
