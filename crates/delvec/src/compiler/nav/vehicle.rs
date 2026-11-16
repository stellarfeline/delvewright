//! **A body that carries another** (spec-0135 §3, §4): where each vehicle is
//! seated, the medium it moves through, where a body boards and leaves it, the
//! proofs over the class (`DW1097`–`DW1099`) and the ride the route proof
//! takes on a leg an objective declares ridden.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{DwCode, ExitTier, VehicleKind, VehicleMedium};

use crate::compiler::failure::Failure;
use crate::compiler::nav::world::World;
use crate::compiler::nav::{Footprint, entity_footprint, player_eye_over};

delvewright_dsl::dw_code! {
    /// `DW1097` (spec-0135 §8): **a vehicle seated outside its medium** — a boat
    /// on a floor, a cart off a rail, a mount in water or in a wall. Build tier.
    pub const DW_VEHICLE_OUT_OF_MEDIUM: DwCode = DwCode::new("DW1097", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW1098` (spec-0135 §8): **a medium cell the vehicle reaches from which
    /// no leaving cell reaches the proven walk region** — a rider who leaves
    /// there is stranded. Build tier.
    pub const DW_VEHICLE_STRANDS: DwCode = DwCode::new("DW1098", ExitTier::Build);
}
delvewright_dsl::dw_code! {
    /// `DW1099` (spec-0135 §8): **a vehicle nobody can board, or that cannot
    /// carry the party** — a mark outside its `bounds`; no boarding cell; or a
    /// ride leg no walk crosses whose boardable seats are fewer than four.
    /// Build tier.
    pub const DW_VEHICLE_UNBOARDABLE: DwCode = DwCode::new("DW1099", ExitTier::Build);
}

delvewright_dsl::dw_code! {
    /// `DW1107` (spec-0135 §3, §8): **a second ride on a vehicle that stays
    /// where its rider left it** (`left: where-left`), with no `seat-vehicle`
    /// the path fires between the two rides — every proof of a ride starts at
    /// the mark, and the body is wherever the last ride left it. Build tier.
    pub const DW_VEHICLE_LEFT_ELSEWHERE: DwCode = DwCode::new("DW1107", ExitTier::Build);
}

/// The party a required ride carries: the engine never proves a party split.
pub const PARTY_SEATS: u32 = 4;

/// The highest rise a mount takes under a rider, in sixteenths: one block,
/// the step the pinned server takes from a rider's client and refuses past
/// (the record §4: a block taken whole, a two-block wall held at its face).
/// Never a jump.
const MOUNT_RISE_16: i64 = 16;

/// A vehicle as the compiler resolved it.
#[derive(Debug, Clone, PartialEq)]
pub struct VehiclePlan {
    /// The declared id.
    pub id: String,
    /// `safe_local` of the id: the tag and the function names.
    pub safe: String,
    /// Index in `vehicles[]`.
    pub index: usize,
    /// The entity.
    pub entity: String,
    /// The kind.
    pub kind: VehicleKind,
    /// The medium.
    pub medium: VehicleMedium,
    /// Riders of the party it seats: the kind's seats, less the one its
    /// actor takes (spec-0135 §13).
    pub seats: u32,
    /// Summoned `Tame`.
    pub tame: bool,
    /// The item a rider holds to steer it.
    pub steering_item: Option<String>,
    /// The mark's cell.
    pub seat: [i32; 3],
    /// The bounds box.
    pub bounds: ([i32; 3], [i32; 3]),
    /// The spawn yaw.
    pub yaw: i32,
    /// The way it faces, as a horizontal step: the way a cart is pushed.
    pub facing: [i32; 2],
    /// When its body is first placed (spec-0135 §2).
    pub seated: delvewright_dsl::VehicleSeated,
    /// Where a riderless body is once its rider has left it (spec-0135 §3).
    pub left: delvewright_dsl::VehicleLeft,
    /// Which of the vehicle's bodies this is, from 0 (spec-0135 §13).
    pub body: u32,
    /// The most bodies the vehicle stands: its count for a full party.
    pub bodies: u32,
    /// The fewest players present at which this body is seated, or `None`
    /// when it is one of the count's `base` and always is (spec-0134 §4's
    /// rule, `emit::party_threshold`).
    pub threshold: Option<u32>,
    /// The declared count.
    pub count: delvewright_dsl::MobCount,
}

impl VehiclePlan {
    /// The tag this body carries alone: the vehicle's tag for a vehicle of
    /// one body, else the vehicle's tag and the body's place.
    pub fn tag(&self) -> String {
        format!("dw_vehicle_{}", self.safe)
    }

    /// The tag every body of this vehicle carries, whatever its count: what a
    /// `riders` census and the ride's server reading ask for.
    pub fn group_tag(&self) -> String {
        format!("dw_vehicle_{}", crate::compiler::plan::safe_local(&self.id))
    }

    /// `safe_local` of the vehicle's id: the name of the functions that seat
    /// and remove every body of it.
    pub fn group_safe(&self) -> String {
        crate::compiler::plan::safe_local(&self.id)
    }

    /// Whether the vehicle stands more than one body.
    pub fn several(&self) -> bool {
        self.bodies > 1
    }

    /// Whether `c` is inside `bounds`.
    pub fn within(&self, c: [i32; 3]) -> bool {
        let (lo, hi) = self.bounds;
        (0..3).all(|i| lo[i] <= c[i] && c[i] <= hi[i])
    }

    /// The vehicle's body footprint.
    pub fn footprint(&self) -> Footprint {
        entity_footprint(&self.entity)
    }

    /// **The cells its body stands in at its mark**: the mark's own column,
    /// two cells high — what a walk to it goes round, never through. A body
    /// that walks into a vehicle shoves it (the record §11): a cart rolls
    /// along its rail, a boat drifts, a mount is pushed off its mark. The
    /// column and not the footprint: a boarding cell is beside the mark, and a
    /// wide body's footprint would hold the very cell it is boarded from.
    pub fn body_cells(&self) -> BTreeSet<[i32; 3]> {
        (0..2)
            .map(|dy| [self.seat[0], self.seat[1] + dy, self.seat[2]])
            .collect()
    }

    /// The rate a ride on it is driven and waited for at, in blocks a second
    /// (spec-0135 §4), each a rate the record measured the pinned server at:
    /// a boat at 5 (§5 took 2, 5, 10 and 20 whole), a cart at the slowest
    /// whole run measured (§11), a mount at 4 — the 0.2 blocks a tick §4 and
    /// §11 saw taken whole, at the mount's own speed and at none, because a
    /// reported move does not read `movement_speed`.
    pub fn rate(&self) -> f64 {
        match self.kind {
            VehicleKind::Boat => 5.0,
            VehicleKind::Minecart => crate::compiler::rail::CART_RATE,
            VehicleKind::Mount => 4.0,
        }
    }

    /// **Where a seated rider's eye is** with the vehicle at medium cell `m`
    /// (spec-0135 §5, the record §15): the vehicle's rest over the cell plus
    /// [`player_eye_over`] — the server judges a rider's block use and
    /// entity interaction from this eye at the ranges a standing player has.
    /// `None` for a kind an act from the seat is not proven on — a minecart
    /// and every mount but the pig — whose rider acts after leaving.
    pub fn rider_eye(&self, world: &World, m: [i32; 3]) -> Option<[f64; 3]> {
        let rest = match self.kind {
            // The record §5: where a boat rests over its water cell's floor.
            VehicleKind::Boat => boat_rest_y(m[1]),
            VehicleKind::Mount if self.entity == "minecraft:pig" => world.feet_y(m),
            VehicleKind::Mount | VehicleKind::Minecart => return None,
        };
        Some([
            f64::from(m[0]) + 0.5,
            rest + player_eye_over(&self.entity),
            f64::from(m[2]) + 0.5,
        ])
    }

    /// **The highest a seated rider's eye can be** with the vehicle at medium
    /// cell `m`, in blocks: the vehicle's rest over the cell plus
    /// [`player_eye_over`]. The rest is the one the record holds, or a bound
    /// over it: a mount on the ground at its feet over the cell; a boat where
    /// its saves rest it (§5); a cart on its rail no higher than the track's
    /// centre on a slope, `0.0625 + 0.5` over the rail cell's floor
    /// (`OldMinecartBehavior.getPos`, §17); a mount in water, whose rest is
    /// not read, anywhere below the top of its water cell.
    fn rider_eye_top(&self, world: &World, m: [i32; 3]) -> f64 {
        let floor = f64::from(m[1]);
        let rest = match (self.kind, self.medium) {
            (VehicleKind::Boat, _) => boat_rest_y(m[1]),
            (VehicleKind::Minecart, _) => floor + CART_REST_OVER_RAIL_CELL,
            (VehicleKind::Mount, VehicleMedium::Water) => floor + 1.0,
            (VehicleKind::Mount, _) => {
                world.feet_16_fp(m, &self.footprint()) as f64
                    / delvewright_dsl::metrics::FULL_16 as f64
            }
        };
        rest + player_eye_over(&self.entity)
    }

    /// **The clear cells over its floor a rider of this vehicle needs at `m`**:
    /// from the medium cell up to the cell its eye can be in.
    pub fn rider_clear_cells(&self, world: &World, m: [i32; 3]) -> i32 {
        (self.rider_eye_top(world, m) - 1e-9).floor() as i32 - m[1] + 1
    }
}

/// How high over its rail cell's floor a cart can rest: the track's centre
/// (`OldMinecartBehavior.getPos`, `0.0625`) raised half a block on a slope
/// (the record §17).
const CART_REST_OVER_RAIL_CELL: f64 = 0.5625;

/// A source water cell's fluid height, as the game holds it: a `float`, 8/9.
const SOURCE_WATER_HEIGHT: f32 = 8.0 / 9.0;

/// How far under its water cell's level a boat comes to rest: fit to the
/// pinned server's save of a boat at rest over a floor at y = 62 (the record
/// §5, `Pos` y 62.52326431274413) by [`boat_rest_y`]'s level.
const BOAT_UNDER_WATER_LEVEL: f64 = 0.365_625;

/// **Where a boat on a source water cell whose floor is at `floor` comes to
/// rest**: the pinned server's save of a boat summoned over its water
/// (`docs/reference/vehicles-animals-and-targets.md` §5), its `Motion` at
/// rest. The rest is a fixed depth under the cell's water level, and the
/// level is the floor plus a source's height summed in `float`, so the rest
/// over the floor moves with the float's precision at that height: 0.52326431274413
/// over a floor at y = 62 and 0.52326049804829 at y = 65, the two saves the
/// record holds, both reproduced to 1e-11 (the fit's residual). A boat
/// summoned there stays there, so the body the summon places is the body the
/// save holds (`DW1040`); a rider's eye stands on it (§15).
pub fn boat_rest_y(floor: i32) -> f64 {
    // `floor` is a block height, exact in a float.
    #[allow(clippy::cast_precision_loss)]
    let level = floor as f32 + SOURCE_WATER_HEIGHT;
    f64::from(level) - BOAT_UNDER_WATER_LEVEL
}

/// **Whether an eye at `eye` acts on `body` at `reach`** (spec-0135 §5): the
/// box's nearest point within `reach` of the eye, and some point of the box
/// in sight past every solid cell but the eye's own and the box's. The rule a
/// standing body is held to ([`crate::compiler::nav::route::stance_cells`]),
/// asked of a rider's eye.
pub fn eye_acts_on(
    world: &World,
    eye: [f64; 3],
    body: &crate::compiler::nav::route::BodyBox,
    reach: f64,
) -> bool {
    crate::compiler::nav::route::eye_point_within(eye, body, reach)
        && crate::compiler::nav::route::eye_point_sees(world, eye, body)
}

/// Every vehicle the campaign declares that the plan can resolve, in
/// declaration order. A vehicle whose entity carries no rider (`DW1096`) or
/// whose anchors do not resolve is skipped; validation names it.
pub fn vehicle_plans(
    campaign: &delvewright_dsl::Campaign,
    anchors: &BTreeMap<(String, String), crate::compiler::plan::ResolvedAnchor>,
) -> Vec<VehiclePlan> {
    let mut out = Vec::new();
    for (index, v) in campaign.quests.content.vehicles.iter().enumerate() {
        let Some(s) = v.steering() else { continue };
        let Some(anchor_cell) = crate::compiler::plan::point_any_in(anchors, v.at.anchor.as_str())
        else {
            continue;
        };
        let Some(bounds) = crate::compiler::plan::zone_box_in(anchors, &v.bounds) else {
            continue;
        };
        // spec-0135 §13: one plan per body, body `k` at the mark plus
        // `k × spacing`; a vehicle of one body is the one plan it always was.
        let bodies = v.count.max();
        for body in 0..bodies {
            let mark = v.at.cell(anchor_cell);
            let off = v.body_offset(body);
            let group = crate::compiler::plan::safe_local(v.id.as_str());
            out.push(VehiclePlan {
                id: v.id.as_str().to_string(),
                safe: if bodies > 1 {
                    format!("{group}_b{body}")
                } else {
                    group
                },
                index,
                body,
                bodies,
                threshold: crate::compiler::emit::party_threshold(v.count, body),
                count: v.count,
                entity: delvewright_dsl::namespaced_entity(&v.entity),
                kind: s.kind,
                medium: s.medium,
                // spec-0135 §13: an actor seated on it takes one seat.
                seats: s.seats.saturating_sub(u32::from(v.rider.is_some())),
                tame: s.tame,
                steering_item: s.steering_item.clone(),
                seat: [mark[0] + off[0], mark[1] + off[1], mark[2] + off[2]],
                bounds,
                yaw: match v.facing {
                    None | Some(delvewright_dsl::Facing::South) => 0,
                    Some(delvewright_dsl::Facing::West) => 90,
                    Some(delvewright_dsl::Facing::North) => 180,
                    Some(delvewright_dsl::Facing::East) => 270,
                },
                facing: match v.facing {
                    None | Some(delvewright_dsl::Facing::South) => [0, 1],
                    Some(delvewright_dsl::Facing::West) => [-1, 0],
                    Some(delvewright_dsl::Facing::North) => [0, -1],
                    Some(delvewright_dsl::Facing::East) => [1, 0],
                },
                seated: v.seated,
                left: v.left,
            });
        }
    }
    out
}

/// **The `riders` censuses a ridden objective waits on** (spec-0135 §5): of
/// the objective's `requires_state`, the terms over a datum that counts the
/// riders of the vehicle its own leg is declared ridden on, and that need at
/// least one aboard — `(datum, riders needed)`. Empty for an objective that
/// declares no ride or waits for no rider.
pub fn riders_gates<'c>(
    campaign: &'c delvewright_dsl::Campaign,
    o: &'c delvewright_dsl::Objective,
) -> Vec<(&'c delvewright_dsl::StateDecl, i32)> {
    let Some(ride) = o.ride() else {
        return Vec::new();
    };
    o.requires_state()
        .iter()
        .filter_map(|cmp| {
            let decl = campaign.quests.content.state_decl(cmp.state.as_str())?;
            let delvewright_dsl::CensusOf::Riders(v) = &decl.counts.as_ref()?.of else {
                return None;
            };
            let need = match cmp.op {
                delvewright_dsl::CompareOp::AtLeast | delvewright_dsl::CompareOp::Equals => {
                    cmp.value
                }
                _ => 0,
            };
            (v == ride && need >= 1).then_some((decl, need))
        })
        .collect()
}

/// One ride the route proof takes (spec-0135 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RideTake {
    /// Index into the plan's vehicles.
    pub vehicle: usize,
    /// Where the party stands to board — or, for a ride that starts aboard,
    /// the medium cell the last ride left it at.
    pub board: [i32; 3],
    /// The medium cells ridden, the start first.
    pub medium: Vec<[i32; 3]>,
    /// Where the party stands after leaving — or, for a ride that stays, the
    /// last medium cell, where the rider performs the act from the seat.
    pub leave: [i32; 3],
    /// The party walks to `board` and mounts; `false` when it is already
    /// aboard at the ride's start (spec-0135 §5).
    pub boards: bool,
    /// The party stays aboard at the ride's end and performs the leg's act
    /// from the seat (spec-0135 §5).
    pub stays: bool,
}

/// Where a ride starts (spec-0135 §4, §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RideFrom {
    /// On foot at this cell: the party walks to a boarding cell and mounts
    /// the vehicle at its mark.
    Foot([i32; 3]),
    /// Aboard, the vehicle at this medium cell: a ride before it ended there
    /// and the party stayed in the seat.
    Aboard([i32; 3]),
}

impl World {
    /// Whether the boat floats at `c`: every footprint column a surface cell
    /// of the water a body swims (spec-0134 §10's relation,
    /// [`World::is_water_surface`]) — a boat does not dive, and floats on
    /// nothing a swimmer is refused.
    fn floats(&self, c: [i32; 3], fp: &Footprint) -> bool {
        fp.cols
            .iter()
            .all(|&[dx, dz]| self.is_water_surface([c[0] + dx, c[1], c[2] + dz]))
    }

    /// Whether a mount stands at `c`: footprint-standable, out of water.
    fn mount_stands(&self, c: [i32; 3], fp: &Footprint) -> bool {
        self.standable_fp(c, fp)
            && fp
                .cols
                .iter()
                .all(|&[dx, dz]| !self.flooded.contains(&[c[0] + dx, c[1], c[2] + dz]))
    }

    /// **Whether a rider of `v` at medium cell `m` has its eye out of every
    /// block**: no cell of the vehicle's columns, from `m` up to the cell its
    /// eye can be in ([`VehiclePlan::rider_clear_cells`]), is solid. The
    /// server hurts a rider whose eye is in a block (`Entity.isInWall`, the
    /// record §17) and nothing else stops the vehicle carrying it there: the
    /// vehicle's own box is all its movement collides.
    pub fn rider_fits(&self, v: &VehiclePlan, m: [i32; 3]) -> bool {
        let clear = v.rider_clear_cells(self, m);
        let fp = v.footprint();
        fp.cols
            .iter()
            .chain(std::iter::once(&[0, 0]))
            .all(|&[dx, dz]| (0..clear).all(|dy| !self.is_solid([m[0] + dx, m[1] + dy, m[2] + dz])))
    }

    /// **Whether the seat is in the vehicle's medium** (`DW1097`).
    pub fn seat_in_medium(&self, v: &VehiclePlan) -> bool {
        let fp = v.footprint();
        match v.medium {
            VehicleMedium::Water => self.floats(v.seat, &fp),
            VehicleMedium::Rails => {
                let solid = |c: [i32; 3]| self.solid.contains(&c);
                crate::compiler::rail::rests(&self.rails, &solid, v.seat)
                    && self.rails[&v.seat]
                        .exits
                        .iter()
                        .any(|(d, _)| *d == v.facing)
                    // Behind it the rail goes on or a block stands: a body
                    // that walks into a cart at the open end of its line
                    // rolls it off the rail, where the boarding push dies on
                    // the floor (the record §11).
                    && {
                        let back = [v.seat[0] - v.facing[0], v.seat[1], v.seat[2] - v.facing[1]];
                        solid(back)
                            || crate::compiler::rail::rail_neighbours(&self.rails, v.seat)
                                .iter()
                                .any(|n| n[0] == back[0] && n[2] == back[2])
                    }
            }
            VehicleMedium::Ground => self.mount_stands(v.seat, &fp),
        }
    }

    /// The medium neighbours of `c` for `v`.
    fn medium_next(&self, v: &VehiclePlan, fp: &Footprint, c: [i32; 3]) -> Vec<[i32; 3]> {
        match v.medium {
            VehicleMedium::Water => [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .into_iter()
                .map(|(dx, dz)| [c[0] + dx, c[1], c[2] + dz])
                .filter(|n| self.floats(*n, fp))
                .collect(),
            VehicleMedium::Ground => {
                let here = self.feet_16_fp(c, fp);
                self.neighbors_walk_fp(c, fp)
                    .into_iter()
                    .filter(|n| {
                        self.feet_16_fp(*n, fp) - here <= MOUNT_RISE_16 && self.mount_stands(*n, fp)
                    })
                    .collect()
            }
            VehicleMedium::Rails => Vec::new(),
        }
    }

    /// **The medium the vehicle reaches from its seat, inside its bounds**, in
    /// breadth-first order with each cell's parent — the medium graph of
    /// spec-0135 §4. A cart's is its driven run ([`crate::compiler::rail::driven_run`]).
    pub fn medium_tree(&self, v: &VehiclePlan) -> BTreeMap<[i32; 3], Option<[i32; 3]>> {
        self.medium_tree_avoiding(v, &|_| false, true)
    }

    /// [`World::medium_tree`] through no cell `avoid` names — the medium a
    /// ride may take on a leg that keeps out of some volume. A cart cannot
    /// stop short of where its rails take it, so a run that meets an avoided
    /// cell is no medium at all: the seat alone. With `headroom`, a cell its
    /// rider does not fit at ([`World::rider_fits`]) is avoided too, and a
    /// seat it does not fit at is no medium; without it, the medium is the
    /// vehicle's alone — asked only to name the cell a refused ride is cut at.
    fn medium_tree_avoiding(
        &self,
        v: &VehiclePlan,
        avoid: &dyn Fn([i32; 3]) -> bool,
        headroom: bool,
    ) -> BTreeMap<[i32; 3], Option<[i32; 3]>> {
        if !self.seat_in_medium(v) || !v.within(v.seat) {
            return BTreeMap::new();
        }
        self.medium_tree_from(v, v.seat, avoid, headroom)
    }

    /// [`World::medium_tree_avoiding`] rooted at `root` — the seat, or the
    /// medium cell a rider who stayed aboard is at.
    fn medium_tree_from(
        &self,
        v: &VehiclePlan,
        root: [i32; 3],
        avoid: &dyn Fn([i32; 3]) -> bool,
        headroom: bool,
    ) -> BTreeMap<[i32; 3], Option<[i32; 3]>> {
        if headroom && !self.rider_fits(v, root) {
            return BTreeMap::new();
        }
        let avoid = |c: [i32; 3]| avoid(c) || (headroom && !self.rider_fits(v, c));
        let mut parent: BTreeMap<[i32; 3], Option<[i32; 3]>> = BTreeMap::new();
        if root != v.seat {
            // Aboard elsewhere: a mount or a boat goes on through its medium;
            // a cart is where its run ended.
            if v.medium == VehicleMedium::Rails {
                parent.insert(root, None);
                return parent;
            }
        }
        if v.medium == VehicleMedium::Rails {
            let within = |c: [i32; 3]| v.within(c);
            let mut run = crate::compiler::rail::driven_run(&self.rails, v.seat, v.facing, &within)
                .unwrap_or_else(|| vec![v.seat]);
            if run.iter().any(|c| avoid(*c)) {
                run = vec![v.seat];
            }
            let mut prev = None;
            for c in run {
                parent.insert(c, prev);
                prev = Some(c);
            }
            return parent;
        }
        let fp = v.footprint();
        let mut queue = VecDeque::from([root]);
        parent.insert(root, None);
        while let Some(c) = queue.pop_front() {
            for n in self.medium_next(v, &fp, c) {
                if v.within(n) && !avoid(n) && !parent.contains_key(&n) {
                    parent.insert(n, Some(c));
                    queue.push_back(n);
                }
            }
        }
        parent
    }

    /// The path through the medium from the seat to `to`, the seat first.
    pub fn medium_path(
        tree: &BTreeMap<[i32; 3], Option<[i32; 3]>>,
        to: [i32; 3],
    ) -> Option<Vec<[i32; 3]>> {
        let mut out = vec![to];
        let mut at = *tree.get(&to)?;
        while let Some(c) = at {
            out.push(c);
            at = tree[&c];
        }
        out.reverse();
        Some(out)
    }

    /// **The boarding cells** (spec-0135 §4): standable cells from which a
    /// body's eye reaches the vehicle's hitbox at its seat within the
    /// interaction reach, nearest the seat first.
    pub fn boarding_cells(&self, v: &VehiclePlan) -> Vec<[i32; 3]> {
        let (w, h) = crate::compiler::nav::entity_dims(&v.entity);
        let r = 4;
        let mut out = Vec::new();
        for dy in -r..=r {
            for dz in -r..=r {
                for dx in -r..=r {
                    let c = [v.seat[0] + dx, v.seat[1] + dy, v.seat[2] + dz];
                    if c != v.seat
                        && self.is_standable(c)
                        && crate::compiler::strand::strikes(self, c, v.seat, w, h)
                    {
                        out.push(c);
                    }
                }
            }
        }
        // Nearest the seat first: a ride boards from the closest cell the
        // party walks to.
        out.sort_by_key(|c| {
            (
                (0..3)
                    .map(|i| i64::from(c[i] - v.seat[i]).pow(2))
                    .sum::<i64>(),
                *c,
            )
        });
        out
    }

    /// **The leaving cells of one medium cell** (spec-0135 §4, the record
    /// §4–§6): standable cells beside it — within a block across and a block
    /// up or down — and, for a mount, the cell it stands in.
    pub fn leaving_cells_of(&self, v: &VehiclePlan, m: [i32; 3]) -> Vec<[i32; 3]> {
        let mut out = Vec::new();
        // A body steps off the vehicle's own edge: a block past its widest
        // column.
        let r = v
            .footprint()
            .cols
            .iter()
            .map(|c| c[0].abs().max(c[1].abs()))
            .max()
            .unwrap_or(0)
            + 1;
        for dy in [0, 1, -1] {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx == 0 && dz == 0 && v.kind != VehicleKind::Mount {
                        continue;
                    }
                    let c = [m[0] + dx, m[1] + dy, m[2] + dz];
                    if self.is_standable(c) && !self.flooded.contains(&c) && !out.contains(&c) {
                        out.push(c);
                    }
                }
            }
        }
        out
    }
}

/// What the vehicle proofs examined (spec-0135 §6), printed on every validate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VehicleBinding {
    /// One row per resolved vehicle.
    pub rows: Vec<VehicleRow>,
    /// Ride legs on the critical path.
    pub critical_rides: usize,
    /// Objectives that declare their leg ridden.
    pub declared: usize,
    /// Per vehicle of several bodies (spec-0135 §13): its id and the bodies
    /// it seats for a party of 1, 2, 3 and 4.
    pub parties: Vec<(String, [u32; 4])>,
}

/// One vehicle's counts.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct VehicleRow {
    /// The id.
    pub id: String,
    /// The kind token.
    pub kind: &'static str,
    /// Medium cells in bounds.
    pub medium: usize,
    /// Boarding cells.
    pub board: usize,
    /// Leaving cells.
    pub leave: usize,
    /// Seats.
    pub seats: u32,
}

impl VehicleBinding {
    /// The binding line.
    pub fn line(&self) -> String {
        let rows = if self.rows.is_empty() {
            String::new()
        } else {
            format!(
                " — {}",
                self.rows
                    .iter()
                    .map(|r| format!(
                        "{} {} medium {} cell(s) in bounds, board {} cell(s), leave {} cell(s), \
                         seats {}",
                        r.kind, r.id, r.medium, r.board, r.leave, r.seats
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };
        let parties = if self.parties.is_empty() {
            String::new()
        } else {
            format!(
                "; bodies per party of 1–4 (DW1115): {}",
                self.parties
                    .iter()
                    .map(|(id, n)| format!("{id} {}, {}, {}, {}", n[0], n[1], n[2], n[3]))
                    .collect::<Vec<_>>()
                    .join("; ")
            )
        };
        format!(
            "vehicle binding (DW1097–DW1099): {} vehicle(s){rows}; {} objective(s) declare \
             their leg ridden; {} ride leg(s) on the critical path{parties}",
            self.rows.len()
                - self
                    .parties
                    .iter()
                    .map(|(_, n)| n[3] as usize - 1)
                    .sum::<usize>(),
            self.declared,
            self.critical_rides
        )
    }
}

/// **The proofs over the class, over a plan's vehicles** — [`judge_vehicles`]
/// with the walk region the party walks (rooted at the plan's anchors) and the
/// binding's ride counts filled from `ridden`, the legs the route proof rode.
/// The one call every door that asks the route proof makes before it reads
/// the route's verdict, so a vehicle out of its medium is named as that
/// (`DW1097`) and not as the leg it could not carry, judged as the walk it is
/// not (`DW0311`): the final build and a world-edits batch both ask here.
pub fn judge_plan_vehicles(
    plan: &crate::compiler::plan::Plan,
    world: &World,
    ridden: usize,
) -> (VehicleBinding, Result<(), Failure>) {
    // The region the party walks; a rider who leaves must get back to it by
    // the reach graph.
    let walk = if plan.vehicles.is_empty() {
        BTreeSet::new()
    } else {
        world.walk_region_rooted(&crate::compiler::edit::anchor_starts(plan))
    };
    let (mut vb, verdict) = judge_vehicles(&plan.vehicles, world, &walk);
    vb.critical_rides = ridden;
    vb.declared = plan.declared_rides.len();
    (vb, verdict)
}

/// **Whether a body set down at some cells gets to the walk region** by the
/// reach graph (`DW1098`): the closure from the cells holds a standing cell
/// the party walks. Asked once per medium cell of a vehicle, so it is
/// answered from what an earlier question already learned: every closure
/// asked keeps the states that reach the walk region over its own edges
/// ([`crate::compiler::nav::world::Reach::reaching`]), and a set holding one
/// of them reaches it too — those edges are moves a body makes. Any other set
/// asks the graph for its own closure, as every question once did, so the
/// answer is the closure's own; a bay is flooded once, and walked backwards
/// once, rather than walked forwards from every cell of it.
pub(crate) struct OffWalk<'w> {
    world: &'w World,
    walk: &'w BTreeSet<[i32; 3]>,
    /// The states known to reach the walk region, over every closure asked.
    reaching: BTreeSet<[i32; 3]>,
    /// How many closures were asked of the graph.
    pub(crate) asked: usize,
}

impl<'w> OffWalk<'w> {
    pub(crate) fn new(world: &'w World, walk: &'w BTreeSet<[i32; 3]>) -> Self {
        Self {
            world,
            walk,
            reaching: BTreeSet::new(),
            asked: 0,
        }
    }

    /// Whether a body at any of `from` gets to a standing cell of the walk
    /// region.
    pub(crate) fn reaches(&mut self, from: &[[i32; 3]]) -> bool {
        if from.iter().any(|c| self.reaching.contains(c)) {
            return true;
        }
        self.asked += 1;
        let r = self.world.body_reach(from);
        let seeds: Vec<[i32; 3]> = r
            .standing()
            .into_iter()
            .filter(|c| self.walk.contains(c))
            .collect();
        if seeds.is_empty() {
            return false;
        }
        // Every state of the closure is reached from `from` over its own
        // edges, so the closure holds a standing cell of the walk region
        // exactly when one of `from` walks back from one.
        let back = r.reaching(seeds, |_| true, true);
        let off = from.iter().any(|c| back.contains(c));
        self.reaching.extend(back);
        off
    }
}

/// **The proofs over the class** (spec-0135 §4): every resolved vehicle's seat
/// is in its medium (`DW1097`), its mark inside its bounds and a boarding cell
/// beside it (`DW1099`), and from every medium cell a leaving cell reaches
/// `walk` — the proven walk region (`DW1098`). Returns the binding beside the
/// first refusal, so the caller prints it either way.
pub fn judge_vehicles(
    vehicles: &[VehiclePlan],
    world: &World,
    walk: &BTreeSet<[i32; 3]>,
) -> (VehicleBinding, Result<(), Failure>) {
    let mut b = VehicleBinding::default();
    let mut verdict = Ok(());
    // One for every vehicle: two boats on one bay ask the graph once.
    let mut off_walk = OffWalk::new(world, walk);
    for v in vehicles {
        if v.several() && v.body == 0 {
            let at = |n: u32| {
                (0..v.bodies)
                    .filter(|k| {
                        crate::compiler::emit::party_threshold(v.count, *k).is_none_or(|t| n >= t)
                    })
                    .count() as u32
            };
            b.parties.push((v.id.clone(), [at(1), at(2), at(3), at(4)]));
        }
        let mut row = VehicleRow {
            id: if v.several() {
                format!("{} body {}", v.id, v.body)
            } else {
                v.id.clone()
            },
            kind: v.kind.token(),
            seats: v.seats,
            ..VehicleRow::default()
        };
        let fail = |code, message: String| Err(Failure { code, message });
        if !v.within(v.seat) {
            if verdict.is_ok() {
                verdict = fail(
                    DW_VEHICLE_UNBOARDABLE,
                    format!(
                        "vehicle `{}` is seated at {:?}, outside its own `bounds` {:?}..{:?} — the \
                         re-seat would return it to a mark it is already outside of, every 100 \
                         ticks. Move the mark inside `bounds`, or grow `bounds` to hold it.",
                        row.id, v.seat, v.bounds.0, v.bounds.1
                    ),
                );
            }
            b.rows.push(row);
            continue;
        }
        if !world.seat_in_medium(v) {
            if verdict.is_ok() {
                let medium = match v.medium {
                    VehicleMedium::Water => "water with open air above, across the boat's width",
                    VehicleMedium::Rails => {
                        "a level rail a cart rests on, running the way the cart faces, with the \
                         rail going on or a solid block standing behind it (a powered rail that \
                         is powered, with a solid block at one end, launches a cart nobody sits \
                         in; a slope rolls it away; at the open end of a line a body that walks \
                         into the cart rolls it off its rail; `facing` is the way the engine \
                         pushes it when a body boards)"
                    }
                    VehicleMedium::Ground => {
                        "ground the mount stands on at its width and height, out of water"
                    }
                };
                verdict = fail(
                    DW_VEHICLE_OUT_OF_MEDIUM,
                    format!(
                        "vehicle `{}` (a `{}`, a {}) is seated at {:?}, which is not {medium} in the \
                         assembled world — a boat on a floor, a cart off a rail, a mount in water \
                         or in a wall is a vehicle nothing can ride. Move the mark into the \
                         medium, or build the medium under it.",
                        row.id,
                        v.entity,
                        v.kind.token(),
                        v.seat
                    ),
                );
            }
            b.rows.push(row);
            continue;
        }
        let tree = world.medium_tree(v);
        row.medium = tree.len();
        let board = world.boarding_cells(v);
        row.board = board.len();
        if board.is_empty() && verdict.is_ok() {
            verdict = fail(
                DW_VEHICLE_UNBOARDABLE,
                format!(
                    "vehicle `{}` at {:?} has no boarding cell: no cell a body stands in puts an eye \
                     within the 3-block interaction reach of its body, so nobody can climb on. \
                     Give it a floor beside it — a jetty, a platform, a paddock gate.",
                    v.id, v.seat
                ),
            );
        }
        let mut leaves: BTreeSet<[i32; 3]> = BTreeSet::new();
        for m in tree.keys() {
            let l = world.leaving_cells_of(v, *m);
            // A rider who leaves is never stranded when a leaving cell is
            // ground the party walks, or a body there gets to such ground by
            // the reach graph — walking, falling, jumping, climbing, swimming
            // (`World::body_reach`). The graph is asked only for a cell the
            // walk region does not already touch, and only until the first
            // refusal ([`OffWalk`]).
            let off = l.iter().any(|c| walk.contains(c)) || {
                // A rider who leaves a boat in open water is in that water
                // (the record §5): the water under the hull is one more place
                // the body starts.
                let mut from = l.clone();
                if v.medium == VehicleMedium::Water {
                    from.push(*m);
                }
                !from.is_empty() && verdict.is_ok() && off_walk.reaches(&from)
            };
            leaves.extend(l);
            if !off && verdict.is_ok() {
                verdict = fail(
                    DW_VEHICLE_STRANDS,
                    format!(
                        "vehicle `{}` reaches {m:?} through its medium, and from no cell a rider \
                         stands on after leaving there does a body get to the region the party \
                         walks — walking, falling, jumping, climbing or swimming — so a rider who \
                         leaves at {m:?} is stranded (a rail into a pit, a far shore of cliff). \
                         Give that stretch a way off, cut it out of `bounds`, or end the medium \
                         before it.",
                        v.id
                    ),
                );
            }
        }
        row.leave = leaves.len();
        b.rows.push(row);
    }
    (b, verdict)
}

/// Why a vehicle carries no ride from one cell to another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RideMiss {
    /// Its seat is outside its medium or its bounds (`DW1097`/`DW1099` name it).
    Unseated,
    /// The party walks to none of its boarding cells from the leg's start.
    NoBoard,
    /// Its medium is its seat alone: it goes nowhere.
    NoMedium,
    /// The party walks to the leg's end from none of the cells it is left at.
    NoLeave,
    /// Its seat, or a cart's whole run, lies in a volume the leg keeps out of:
    /// a rider there would complete an armed objective out of turn.
    KeptOut,
    /// Its medium reaches no cell of a checkpoint of the course the leg
    /// runs, from the checkpoint before it (spec-0139 §6).
    OffCourse,
    /// The vehicle alone carries the leg, and at `cell` of that ride its
    /// rider's eye is in a block: the ride needs `clear` cells free of solid
    /// blocks over the cell's floor there.
    Headroom {
        /// The first medium cell of the ride its rider does not fit at.
        cell: [i32; 3],
        /// The clear cells over that cell's floor the rider needs.
        clear: i32,
    },
}

impl RideMiss {
    /// The miss in a diagnostic's words.
    pub fn words(self) -> std::borrow::Cow<'static, str> {
        let fixed = match self {
            RideMiss::Headroom { cell, clear } => {
                return format!(
                    "the vehicle alone fits through {cell:?}, and its rider does not: a player \
                     riding it has its eye in a block there and suffocates. The ride needs \
                     {clear} clear blocks over the floor at {cell:?}; raise the ceiling there, \
                     or keep the medium out of it"
                )
                .into();
            }
            RideMiss::Unseated => "its mark is outside its medium or its `bounds`",
            RideMiss::NoBoard => {
                "the party walks from the leg's start to no cell it can be boarded from"
            }
            RideMiss::NoMedium => {
                "its medium inside `bounds` is its mark alone, so a ride on it goes nowhere"
            }
            RideMiss::KeptOut => {
                "its mark, or the run its rails carry it on, lies within reach of the completion \
                 volume of an objective armed on this leg, so a rider would complete that \
                 objective from the saddle; end the medium, or move the objective's anchor, so \
                 the ride stops short and the party walks in"
            }
            RideMiss::OffCourse => {
                "the leg runs a race's course, and its medium inside `bounds` reaches no cell \
                 of one of the checkpoints from the one before it; build the medium through \
                 every checkpoint in order, widen `bounds`, or move the checkpoint onto it"
            }
            RideMiss::NoLeave => {
                "the party walks to the leg's end from no cell a rider stands on after leaving it, \
                 wherever in its medium the ride ends"
            }
        };
        fixed.into()
    }
}

/// How many leaving cells a ride tries, nearest the leg's end first.
const RIDE_TRIES: usize = 256;

/// **The ride vehicle `vi` carries from `from` to `goal`** (spec-0135 §4, §5),
/// over the leg's world. On foot, the party walks from `from` to a boarding
/// cell and rides the medium from the seat; aboard, it rides on from the
/// medium cell it is at. Where the leg's act can be done from the seat
/// (`seat_act`, asked of a medium cell), the ride ends at the nearest medium
/// cell — the shortest ride, then cell order — from which the rider performs
/// it, and the party stays aboard. Otherwise the party leaves at a cell from
/// which `goal` is walked, **as near the leg's end as the medium reaches**:
/// among the leaving cells that walk on to `goal`, the one nearest it wins,
/// then the shorter ride, then cell order. A ride from the seat moves the
/// vehicle — a medium cell other than the seat — and a cart is left only
/// where its run ends, because the rails take it there. `walks(a, b)` is the
/// leg proof's own walk.
///
/// The ride is proven for its rider as well as its vehicle: no medium cell
/// it passes puts the rider's eye in a block ([`World::rider_fits`]). A ride
/// refused where the vehicle alone would have carried the leg is refused
/// naming the first cell of that ride its rider does not fit at
/// ([`RideMiss::Headroom`]).
#[allow(clippy::too_many_arguments)]
pub fn ride_of(
    world: &World,
    vi: usize,
    v: &VehiclePlan,
    from: RideFrom,
    walks: &dyn Fn([i32; 3], [i32; 3]) -> bool,
    goal: [i32; 3],
    seat_act: Option<&dyn Fn([i32; 3]) -> bool>,
    course: &[crate::compiler::nav::race::CellBox],
) -> Result<RideTake, RideMiss> {
    let ride = |headroom| ride_with(world, vi, v, from, walks, goal, seat_act, course, headroom);
    match ride(true) {
        Err(miss @ (RideMiss::Unseated | RideMiss::NoBoard)) => Err(miss),
        Err(miss) => Err(ride(false).ok().map_or(miss, |take| {
            take.medium
                .iter()
                .find(|m| !world.rider_fits(v, **m))
                .map_or(miss, |&cell| RideMiss::Headroom {
                    cell,
                    clear: v.rider_clear_cells(world, cell),
                })
        })),
        ok => ok,
    }
}

/// [`ride_of`], with the rider's headroom proven or (`headroom` false) the
/// vehicle's body alone.
#[allow(clippy::too_many_arguments)]
fn ride_with(
    world: &World,
    vi: usize,
    v: &VehiclePlan,
    from: RideFrom,
    walks: &dyn Fn([i32; 3], [i32; 3]) -> bool,
    goal: [i32; 3],
    seat_act: Option<&dyn Fn([i32; 3]) -> bool>,
    course: &[crate::compiler::nav::race::CellBox],
    headroom: bool,
) -> Result<RideTake, RideMiss> {
    if !world.seat_in_medium(v) || !v.within(v.seat) {
        return Err(RideMiss::Unseated);
    }
    // The leg keeps out of every completion volume armed on it, its own
    // objective's included (`World::keep_out`), by the hitbox rule a walk is
    // held to: a rider is a body at the seat, so no cell the ride boards
    // from, passes or leaves at may meet one. The ride ends short of its
    // objective's volume and the party walks in.
    let body = Footprint::player();
    let kept = |c: [i32; 3]| world.meets_keep_out_fp(c, &body);
    let (root, board, boards) = match from {
        RideFrom::Foot(start) => {
            if kept(v.seat) {
                return Err(RideMiss::KeptOut);
            }
            let Some(board) = world
                .boarding_cells(v)
                .into_iter()
                .find(|c| !kept(*c) && walks(start, *c))
            else {
                return Err(RideMiss::NoBoard);
            };
            (v.seat, board, true)
        }
        RideFrom::Aboard(at) => (at, at, false),
    };
    // spec-0139 §6: a ride that runs a course goes first through one medium
    // cell of each checkpoint, in order — the nearest along the medium from
    // where the last left it — and the ride proper is planned on from there.
    let mut lead: Vec<[i32; 3]> = Vec::new();
    let mut root = root;
    // A cart is not steered: its rails carry it along one run, so its course
    // is that run, and the checkpoints lie on it in order or the leg is off
    // its course. The ride is then the run it always was.
    let staged: &[crate::compiler::nav::race::CellBox] = if v.medium == VehicleMedium::Rails {
        let tree = if boards {
            world.medium_tree_avoiding(v, &kept, headroom)
        } else {
            world.medium_tree_from(v, root, &kept, headroom)
        };
        let run = tree
            .keys()
            .filter_map(|m| World::medium_path(&tree, *m))
            .max_by_key(Vec::len)
            .unwrap_or_default();
        let mut at = 0;
        for b in course {
            let Some(k) = run[at..]
                .iter()
                .position(|m| crate::compiler::nav::race::cell_in_box(*m, b))
            else {
                return Err(RideMiss::OffCourse);
            };
            at += k;
        }
        &[]
    } else {
        course
    };
    for b in staged {
        let tree = if boards && lead.is_empty() {
            world.medium_tree_avoiding(v, &kept, headroom)
        } else {
            world.medium_tree_from(v, root, &kept, headroom)
        };
        let Some((_, stop)) = tree
            .keys()
            .filter(|m| crate::compiler::nav::race::cell_in_box(**m, b))
            .filter_map(|m| Some((World::medium_path(&tree, *m)?.len(), *m)))
            .min()
        else {
            return Err(RideMiss::OffCourse);
        };
        let path = World::medium_path(&tree, stop).ok_or(RideMiss::OffCourse)?;
        lead.extend(path.into_iter().skip(usize::from(!lead.is_empty())));
        root = stop;
    }
    let tree = if boards && lead.is_empty() {
        world.medium_tree_avoiding(v, &kept, headroom)
    } else {
        world.medium_tree_from(v, root, &kept, headroom)
    };
    // A ride from the seat moves the vehicle; one that goes on from aboard
    // — or from the last checkpoint of a course — may act, or leave, where
    // it is.
    let shortest = if boards && lead.is_empty() { 2 } else { 1 };
    let mut longest = 0usize;
    let mut rides: Vec<(usize, [i32; 3])> = Vec::new();
    for m in tree.keys() {
        let len = World::medium_path(&tree, *m).map_or(0, |p| p.len());
        if len >= shortest {
            longest = longest.max(len);
            rides.push((len, *m));
        }
    }
    if rides.is_empty() {
        // A cart whose run was refused for the volume it enters, not for
        // going nowhere.
        return Err(
            if tree.len() < world.medium_tree_avoiding(v, &|_| false, headroom).len() {
                RideMiss::KeptOut
            } else {
                RideMiss::NoMedium
            },
        );
    }
    let take = |m: [i32; 3], leave: [i32; 3], stays: bool| -> Result<RideTake, RideMiss> {
        let tail = World::medium_path(&tree, m).ok_or(RideMiss::NoMedium)?;
        let mut medium = lead.clone();
        medium.extend(tail.into_iter().skip(usize::from(!lead.is_empty())));
        Ok(RideTake {
            vehicle: vi,
            board,
            medium,
            leave,
            boards,
            stays,
        })
    };
    let ends_here = |len: usize| v.kind != VehicleKind::Minecart || len == longest;
    if let Some(acts) = seat_act
        && let Some(&(_, m)) = rides
            .iter()
            .filter(|(len, m)| ends_here(*len) && acts(*m))
            .min()
    {
        return take(m, m, true);
    }
    // Leaving cell → the shortest ride that reaches it.
    let mut best: BTreeMap<[i32; 3], (usize, [i32; 3])> = BTreeMap::new();
    for &(len, m) in &rides {
        if !ends_here(len) {
            continue;
        }
        for l in world.leaving_cells_of(v, m) {
            // Nor on the goal's own cell: what the party came for stands
            // there — a body it talks to, a thing it takes.
            if kept(l) || l == goal {
                continue;
            }
            let e = best.entry(l).or_insert((len, m));
            if (len, m) < *e {
                *e = (len, m);
            }
        }
    }
    let far = |c: [i32; 3]| -> i64 {
        (0..3)
            .map(|i| i64::from(c[i] - goal[i]).pow(2))
            .sum::<i64>()
    };
    let mut cands: Vec<(i64, usize, [i32; 3], [i32; 3])> = best
        .into_iter()
        .map(|(l, (len, m))| (far(l), len, l, m))
        .collect();
    cands.sort();
    for (_, _, leave, m) in cands.into_iter().take(RIDE_TRIES) {
        if walks(leave, goal) {
            return take(m, leave, false);
        }
    }
    Err(RideMiss::NoLeave)
}

/// Whether a rider of a vehicle at a medium cell does the leg's act from the
/// seat.
pub type SeatActOf<'a> = dyn Fn(&VehiclePlan, [i32; 3]) -> bool + 'a;

/// The seats every vehicle a ride leg's party can board carries
/// (`DW1099`'s last arm): the seats of every vehicle with a boarding cell the
/// party walks to from `from` and whose ride reaches `goal` — by leaving, or
/// by the act `seat_act` asks after done from its seat. The leg's own vehicle
/// `first` is counted first, and the count stops at [`PARTY_SEATS`]: the
/// rule asks whether the party is seated, and a vehicle that carries no
/// leg tries every leaving cell before it says so. Under four, every vehicle
/// was asked and the count is exact.
pub fn boardable_seats(
    world: &World,
    vehicles: &[VehiclePlan],
    first: usize,
    from: [i32; 3],
    walks: &dyn Fn([i32; 3], [i32; 3]) -> bool,
    goal: [i32; 3],
    seat_act: Option<&SeatActOf<'_>>,
) -> u32 {
    let order = std::iter::once(first)
        .filter(|&i| i < vehicles.len())
        .chain((0..vehicles.len()).filter(|&i| i != first));
    let mut seats = 0;
    for i in order {
        if seats >= PARTY_SEATS {
            break;
        }
        let v = &vehicles[i];
        let acts = seat_act.map(|f| move |m: [i32; 3]| f(v, m));
        if ride_of(
            world,
            i,
            v,
            RideFrom::Foot(from),
            walks,
            goal,
            acts.as_ref().map(|f| f as &dyn Fn([i32; 3]) -> bool),
            &[],
        )
        .is_ok()
        {
            seats += v.seats;
        }
    }
    seats
}

#[cfg(test)]
mod tests {
    /// The rest [`boat_rest_y`] gives is the rest each save the record holds
    /// measured: y = 62 (the record §5) and y = 65 (the gallery's skiff, the
    /// pinned server's save written by `validation/world-save.sh`).
    #[test]
    fn a_boat_rests_where_each_measured_save_holds_it() {
        for (floor, saved) in [(62, 62.523_264_312_744_13), (65, 65.523_260_498_048_29)] {
            let y = super::boat_rest_y(floor);
            assert!(
                (y - saved).abs() < 1e-9,
                "floor {floor}: {y} against {saved}"
            );
        }
    }

    use super::*;

    fn pond(width: i32) -> World {
        // A stone floor at y=0 from x=-5..=width+5, z=-3..=3; water at y=1
        // over x in 0..width, walls the walk cannot cross: the water cells.
        let mut solid = BTreeSet::new();
        let mut flooded = BTreeSet::new();
        for x in -5..=width + 5 {
            for z in -3..=3 {
                solid.insert([x, 0, z]);
                if (0..width).contains(&x) {
                    flooded.insert([x, 1, z]);
                    flooded.insert([x, 2, z]);
                    solid.insert([x, 1, z]);
                    solid.remove(&[x, 1, z]);
                }
            }
        }
        // Deepen the pond so a body sinks and cannot wade: water two deep.
        for x in 0..width {
            for z in -3..=3 {
                solid.remove(&[x, 0, z]);
                flooded.insert([x, 0, z]);
                solid.insert([x, -1, z]);
            }
        }
        for c in flooded.clone() {
            if c[1] == 2 {
                flooded.remove(&c);
            }
        }
        World::from_solid_and_flooded(solid, flooded)
    }

    fn boat(seat: [i32; 3], bounds: ([i32; 3], [i32; 3])) -> VehiclePlan {
        VehiclePlan {
            id: "vehicle/b".into(),
            safe: "b".into(),
            index: 0,
            body: 0,
            bodies: 1,
            threshold: None,
            count: delvewright_dsl::MobCount::Fixed(1),
            entity: "minecraft:oak_boat".into(),
            kind: VehicleKind::Boat,
            medium: VehicleMedium::Water,
            seats: 2,
            tame: false,
            steering_item: None,
            seat,
            bounds,
            yaw: 0,
            facing: [0, 1],
            seated: Default::default(),
            left: Default::default(),
        }
    }

    #[test]
    fn a_boat_floats_on_the_pond_and_not_on_the_floor() {
        let w = pond(8);
        assert!(w.seat_in_medium(&boat([3, 1, 0], ([-10, -5, -5], [20, 5, 5]))));
        assert!(!w.seat_in_medium(&boat([-3, 1, 0], ([-10, -5, -5], [20, 5, 5]))));
    }

    #[test]
    fn a_ride_nobody_walks_to_and_one_that_lands_nowhere_are_named() {
        let w = pond(8);
        let b = boat([1, 1, 0], ([-10, -5, -5], [20, 5, 5]));
        let (from, goal) = ([-3, 1, 0], [10, 1, 0]);
        assert_eq!(
            ride_of(
                &w,
                0,
                &b,
                RideFrom::Foot(from),
                &|_, _| false,
                goal,
                None,
                &[]
            ),
            Err(RideMiss::NoBoard)
        );
        // Boarded, and no leaving cell walks on to the goal.
        assert_eq!(
            ride_of(
                &w,
                0,
                &b,
                RideFrom::Foot(from),
                &|a, _| a == from,
                goal,
                None,
                &[],
            ),
            Err(RideMiss::NoLeave)
        );
        let ride = ride_of(
            &w,
            0,
            &b,
            RideFrom::Foot(from),
            &|_, _| true,
            goal,
            None,
            &[],
        )
        .expect("a ride");
        assert_eq!(ride.medium[0], b.seat);
        assert!(ride.leave[0] >= 8, "left on the far shore: {ride:?}");
        // Two boats at one dock seat four; one seats two.
        let two = [b.clone(), boat([1, 1, 2], b.bounds)];
        assert_eq!(
            boardable_seats(&w, &two, 0, from, &|_, _| true, goal, None),
            4
        );
        assert_eq!(
            boardable_seats(&w, &two[..1], 0, from, &|_, _| true, goal, None),
            2
        );
    }

    /// `DW1098` asks, of every medium cell, whether a rider set down there
    /// gets to the walk region. The bay is one closure: the graph is asked
    /// once for it, not once per cell, and every answer is the one the cell's
    /// own closure gives.
    #[test]
    fn the_way_off_a_bay_is_asked_of_the_graph_once() {
        let w = pond(12);
        let b = boat([3, 1, 0], ([-10, -5, -5], [20, 5, 5]));
        let walk = w.walk_region(&[[-3, 1, 0]]);
        let tree = w.medium_tree(&b);
        assert!(tree.len() > 20, "a bay of many cells: {}", tree.len());
        let mut off = OffWalk::new(&w, &walk);
        for m in tree.keys() {
            let mut from = w.leaving_cells_of(&b, *m);
            from.push(*m);
            let own = w
                .body_reach(&from)
                .standing()
                .iter()
                .any(|c| walk.contains(c));
            assert!(own, "every cell of the pond swims ashore: {m:?}");
            assert_eq!(off.reaches(&from), own, "{m:?}");
        }
        assert_eq!(off.asked, 1, "the graph asked once over the bay");
        // A set nothing in the walk region is reached from answers no.
        let island: BTreeSet<[i32; 3]> = BTreeSet::new();
        let mut none = OffWalk::new(&w, &island);
        assert!(!none.reaches(&[[3, 1, 0]]));
    }

    /// A stone floor at y = 0 over x in -3..=15, z in -2..=2, and a lintel
    /// one block over the floor at x = 6, across the whole width: a pig
    /// (0.9 high) walks under it, a body standing on it does not, and a
    /// player riding it would have its eye in the lintel.
    fn lane_under_a_lintel(lintel: bool) -> World {
        let mut solid = BTreeSet::new();
        for x in -3..=15 {
            for z in -2..=2 {
                solid.insert([x, 0, z]);
                if lintel && x == 6 {
                    solid.insert([x, 2, z]);
                }
            }
        }
        World::from_solid_and_flooded(solid, BTreeSet::new())
    }

    fn mount(entity: &str, seat: [i32; 3]) -> VehiclePlan {
        VehiclePlan {
            entity: entity.into(),
            kind: VehicleKind::Mount,
            medium: VehicleMedium::Ground,
            seats: 1,
            steering_item: Some("minecraft:carrot_on_a_stick".into()),
            ..boat(seat, ([-3, 0, -2], [15, 4, 2]))
        }
    }

    /// The ride of a pig from x = 0 to the far end of the lane, past the
    /// lintel at x = 6, leaving at x >= 10.
    fn ride_past_the_lintel(w: &World, v: &VehiclePlan) -> Result<RideTake, RideMiss> {
        let from = [-2, 1, 0];
        let goal = [14, 1, 0];
        ride_of(
            w,
            0,
            v,
            RideFrom::Foot(from),
            &|a, _| a == from || a[0] >= 10,
            goal,
            None,
            &[],
        )
    }

    /// **A ride is proven for its rider's head, not only the mount's body**:
    /// a pig fits under a lintel one block over the floor, and the player on
    /// it, whose eye stands 1.88875 over the pig's feet, would have that eye
    /// in the lintel and suffocate. The ride is refused naming the cell; with
    /// the lintel gone it is the ride it always was.
    #[test]
    fn a_ride_under_a_lintel_the_mount_fits_and_its_rider_does_not_is_refused() {
        let pig = mount("minecraft:pig", [0, 1, 0]);
        let open = lane_under_a_lintel(false);
        let ride = ride_past_the_lintel(&open, &pig).expect("an open lane rides");
        assert!(ride.medium.contains(&[6, 1, 0]), "{ride:?}");
        let low = lane_under_a_lintel(true);
        assert!(
            low.standable_fp([6, 1, 0], &pig.footprint()),
            "the pig itself stands under the lintel"
        );
        assert!(!low.is_standable([6, 1, 0]), "a standing body does not");
        assert_eq!(
            ride_past_the_lintel(&low, &pig),
            Err(RideMiss::Headroom {
                cell: [6, 1, 0],
                clear: 2
            })
        );
    }

    /// **The clear cells over its floor a rider needs, per vehicle the
    /// pinned game seats a player on** (the record §17): every row of the
    /// steering table is named here, so a vehicle the table gains without a
    /// reading of its passenger point reds this test rather than taking the
    /// fallback unread. A pig's rider fits a two-block door; a horse's needs
    /// three; a camel's four.
    #[test]
    fn every_vehicle_a_player_rides_names_the_clear_cells_its_rider_needs() {
        let mut solid = BTreeSet::new();
        for x in -3..=3 {
            for z in -3..=3 {
                solid.insert([x, 0, z]);
            }
        }
        let w = World::from_solid_and_flooded(solid, BTreeSet::new());
        let expected = |id: &str| -> Option<i32> {
            Some(match id.strip_prefix("minecraft:")? {
                "pig" | "minecart" => 2,
                "horse" | "donkey" | "mule" | "skeleton_horse" | "zombie_horse" | "strider" => 3,
                "camel" | "camel_husk" | "nautilus" | "zombie_nautilus" => 4,
                "bamboo_raft" => 3,
                b if b.ends_with("_boat") => 2,
                _ => return None,
            })
        };
        let mut named = 0;
        for (id, row) in delvewright_dsl::steering_table() {
            let v = VehiclePlan {
                entity: id.clone(),
                kind: row.kind,
                medium: row.medium,
                ..boat([0, 1, 0], ([-3, 0, -3], [3, 4, 3]))
            };
            let want = expected(id).unwrap_or_else(|| panic!("{id}: no reading"));
            assert_eq!(v.rider_clear_cells(&w, [0, 1, 0]), want, "{id}");
            named += 1;
        }
        assert_eq!(named, delvewright_dsl::steering_table().len());
        assert!(named >= 22, "the table holds every vehicle: {named}");
    }

    /// The seat every stacked body reads, against the record's readings: a
    /// pig's rider 0.27 over its feet (§4), a boat's 0.4125 under its own
    /// (§15's 100.52 + 0.1875 - 0.6), the mounted spear zombie 0.61875 over
    /// its zombie horse's (§16); and the box an actor rides in, from the
    /// carrier's feet to the higher top.
    #[test]
    fn a_rider_stands_where_the_record_reads_it() {
        use crate::compiler::nav::{ridden_dims, rider_feet_over};
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(near(
            rider_feet_over("minecraft:player", "minecraft:pig"),
            0.268_75
        ));
        assert!(near(
            rider_feet_over("minecraft:player", "minecraft:oak_boat"),
            -0.4125
        ));
        assert!(near(
            rider_feet_over("minecraft:zombie", "minecraft:zombie_horse"),
            0.618_75
        ));
        let (w, h) = ridden_dims("minecraft:mannequin", Some("minecraft:pig"));
        assert!(near(w, 0.9) && near(h, 2.068_75), "{w} x {h}");
        let (w, h) = ridden_dims("minecraft:zombie", Some("minecraft:zombie_horse"));
        assert!(near(w, 1.396_484_4) && near(h, 2.568_75), "{w} x {h}");
        // A rider wholly under its carrier's top leaves the carrier's box.
        let (_, h) = ridden_dims("minecraft:silverfish", Some("minecraft:camel"));
        assert!(near(h, 2.375), "{h}");
    }

    #[test]
    fn the_medium_is_cut_at_bounds() {
        let w = pond(8);
        let all = w.medium_tree(&boat([3, 1, 0], ([-10, -5, -5], [20, 5, 5])));
        let cut = w.medium_tree(&boat([3, 1, 0], ([0, -5, -5], [4, 5, 5])));
        assert!(cut.len() < all.len() && !cut.is_empty());
    }
}
