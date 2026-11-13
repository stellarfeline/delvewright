//! **A body that carries another** (spec-0135 §3, §4): where each vehicle is
//! seated, the medium it moves through, where a body boards and leaves it, the
//! proofs over the class (`DW1097`–`DW1099`) and the ride the route proof
//! takes on a leg an objective declares ridden.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use delvewright_dsl::{DwCode, ExitTier, VehicleKind, VehicleMedium};

use crate::compiler::failure::Failure;
use crate::compiler::nav::world::World;
use crate::compiler::nav::{Footprint, entity_footprint};

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
    /// Riders it seats.
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
}

impl VehiclePlan {
    /// The tag every body of this vehicle carries.
    pub fn tag(&self) -> String {
        format!("dw_vehicle_{}", self.safe)
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
    /// (spec-0135 §5, the record §15): the vehicle's rest over the cell, plus
    /// its passenger attachment, less the player's vehicle attachment, plus
    /// the player's standing eye — the server judges a rider's block use and
    /// entity interaction from this eye at the ranges a standing player has.
    /// `None` for a kind whose rest or attachment the record does not hold —
    /// a minecart and every mount but the pig — whose rider acts after
    /// leaving.
    pub fn rider_eye(&self, world: &World, m: [i32; 3]) -> Option<[f64; 3]> {
        let rest = match self.kind {
            // The record §5: where a boat rests over its water cell's floor.
            VehicleKind::Boat => f64::from(m[1]) + BOAT_AFLOAT,
            VehicleKind::Mount if self.entity == "minecraft:pig" => world.feet_y(m),
            VehicleKind::Mount | VehicleKind::Minecart => return None,
        };
        let attach = match self.kind {
            VehicleKind::Boat => {
                let (_, h) = crate::compiler::nav::entity_dims(&self.entity);
                if self.entity.ends_with("_raft") {
                    h * RAFT_PASSENGER_OVER_HEIGHT
                } else {
                    h * BOAT_PASSENGER_OVER_HEIGHT
                }
            }
            _ => PIG_PASSENGER_ATTACHMENT,
        };
        Some([
            f64::from(m[0]) + 0.5,
            rest + attach - PLAYER_VEHICLE_ATTACHMENT + delvewright_dsl::metrics::PLAYER_EYE_HEIGHT,
            f64::from(m[2]) + 0.5,
        ])
    }
}

/// **Where a boat on a source water cell comes to rest**, over the cell's
/// floor: the pinned server's save of a boat summoned over its water
/// (`docs/reference/vehicles-animals-and-targets.md` §5), its `Motion` at
/// rest. A boat summoned there stays there, so the body the summon places is
/// the body the save holds (`DW1040`); a rider's eye stands on it (§15).
pub const BOAT_AFLOAT: f64 = 0.523_264_312_744_13;

/// A boat's passenger attachment over its feet, as a share of its height:
/// `Boat.getPassengerAttachmentPoint`, `dimensions.height() / 3.0F` (the
/// record §15).
const BOAT_PASSENGER_OVER_HEIGHT: f64 = 1.0 / 3.0;

/// A raft's: `Raft`, `dimensions.height() * 0.8888889F` (the record §15).
const RAFT_PASSENGER_OVER_HEIGHT: f64 = 0.888_888_9;

/// A pig's passenger attachment over its feet: `EntityType.PIG`'s
/// `passengerAttachments(0.86875F)` (the record §15).
const PIG_PASSENGER_ATTACHMENT: f64 = 0.868_75;

/// The player's vehicle attachment: `Avatar.DEFAULT_VEHICLE_ATTACHMENT`,
/// `(0, 0.6, 0)` — a rider's feet stand this far under the vehicle's
/// passenger point (the record §15).
const PLAYER_VEHICLE_ATTACHMENT: f64 = 0.6;

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
        out.push(VehiclePlan {
            id: v.id.as_str().to_string(),
            safe: crate::compiler::plan::safe_local(v.id.as_str()),
            index,
            entity: delvewright_dsl::namespaced_entity(&v.entity),
            kind: s.kind,
            medium: s.medium,
            seats: s.seats,
            tame: s.tame,
            steering_item: s.steering_item.clone(),
            seat: v.at.cell(anchor_cell),
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
        self.medium_tree_avoiding(v, &|_| false)
    }

    /// [`World::medium_tree`] through no cell `avoid` names — the medium a
    /// ride may take on a leg that keeps out of some volume. A cart cannot
    /// stop short of where its rails take it, so a run that meets an avoided
    /// cell is no medium at all: the seat alone.
    fn medium_tree_avoiding(
        &self,
        v: &VehiclePlan,
        avoid: &dyn Fn([i32; 3]) -> bool,
    ) -> BTreeMap<[i32; 3], Option<[i32; 3]>> {
        if !self.seat_in_medium(v) || !v.within(v.seat) {
            return BTreeMap::new();
        }
        self.medium_tree_from(v, v.seat, avoid)
    }

    /// [`World::medium_tree_avoiding`] rooted at `root` — the seat, or the
    /// medium cell a rider who stayed aboard is at.
    fn medium_tree_from(
        &self,
        v: &VehiclePlan,
        root: [i32; 3],
        avoid: &dyn Fn([i32; 3]) -> bool,
    ) -> BTreeMap<[i32; 3], Option<[i32; 3]>> {
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
        format!(
            "vehicle binding (DW1097–DW1099): {} vehicle(s){rows}; {} objective(s) declare \
             their leg ridden; {} ride leg(s) on the critical path",
            self.rows.len(),
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
        let mut row = VehicleRow {
            id: v.id.clone(),
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
                        v.id, v.seat, v.bounds.0, v.bounds.1
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
                        v.id,
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
}

impl RideMiss {
    /// The miss in a diagnostic's words.
    pub fn words(self) -> &'static str {
        match self {
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
            RideMiss::NoLeave => {
                "the party walks to the leg's end from no cell a rider stands on after leaving it, \
                 wherever in its medium the ride ends"
            }
        }
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
pub fn ride_of(
    world: &World,
    vi: usize,
    v: &VehiclePlan,
    from: RideFrom,
    walks: &dyn Fn([i32; 3], [i32; 3]) -> bool,
    goal: [i32; 3],
    seat_act: Option<&dyn Fn([i32; 3]) -> bool>,
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
    let tree = if boards {
        world.medium_tree_avoiding(v, &kept)
    } else {
        world.medium_tree_from(v, root, &kept)
    };
    // A ride from the seat moves the vehicle; one that goes on from aboard
    // may act, or leave, where it is.
    let shortest = if boards { 2 } else { 1 };
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
        return Err(if tree.len() < world.medium_tree(v).len() {
            RideMiss::KeptOut
        } else {
            RideMiss::NoMedium
        });
    }
    let take = |m: [i32; 3], leave: [i32; 3], stays: bool| -> Result<RideTake, RideMiss> {
        Ok(RideTake {
            vehicle: vi,
            board,
            medium: World::medium_path(&tree, m).ok_or(RideMiss::NoMedium)?,
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
            ride_of(&w, 0, &b, RideFrom::Foot(from), &|_, _| false, goal, None),
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
                None
            ),
            Err(RideMiss::NoLeave)
        );
        let ride =
            ride_of(&w, 0, &b, RideFrom::Foot(from), &|_, _| true, goal, None).expect("a ride");
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

    #[test]
    fn the_medium_is_cut_at_bounds() {
        let w = pond(8);
        let all = w.medium_tree(&boat([3, 1, 0], ([-10, -5, -5], [20, 5, 5])));
        let cut = w.medium_tree(&boat([3, 1, 0], ([0, -5, -5], [4, 5, 5])));
        assert!(cut.len() < all.len() && !cut.is_empty());
    }
}
