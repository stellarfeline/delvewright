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
        });
    }
    out
}

/// One ride the route proof takes (spec-0135 §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RideTake {
    /// Index into the plan's vehicles.
    pub vehicle: usize,
    /// Where the party stands to board.
    pub board: [i32; 3],
    /// The medium cells ridden, the seat first.
    pub medium: Vec<[i32; 3]>,
    /// Where the party stands after leaving.
    pub leave: [i32; 3],
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
        let mut parent: BTreeMap<[i32; 3], Option<[i32; 3]>> = BTreeMap::new();
        if !self.seat_in_medium(v) || !v.within(v.seat) {
            return parent;
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
        let mut queue = VecDeque::from([v.seat]);
        parent.insert(v.seat, None);
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
            // (`World::body_reach`). A rider who leaves a boat in open water
            // is in that water (the record §5), so the water under the hull
            // is one more place the flood starts. The flood is run only for a
            // cell the walk region does not already touch, and only until the
            // first refusal; the graph keeps a closure it already took, so a
            // bay is flooded once.
            let off = l.iter().any(|c| walk.contains(c)) || {
                let mut from = l.clone();
                if v.medium == VehicleMedium::Water {
                    from.push(*m);
                }
                !from.is_empty()
                    && verdict.is_ok()
                    && world
                        .body_reach(&from)
                        .standing()
                        .iter()
                        .any(|c| walk.contains(c))
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

/// **The ride vehicle `vi` carries from `from` to `goal`** (spec-0135 §4), over
/// the leg's world: the party walks from `from` to a boarding cell, rides the
/// medium from the seat, and leaves at a cell from which `goal` is walked. A
/// ride takes the party **as near the leg's end as the medium reaches**: among
/// the leaving cells that walk on to `goal`, the one nearest it wins, then the
/// shorter ride, then cell order. A ride moves the vehicle — a medium cell
/// other than the seat — and a cart is left only where its run ends, because
/// the rails take it there. `walks(a, b)` is the leg proof's own walk.
pub fn ride_of(
    world: &World,
    vi: usize,
    v: &VehiclePlan,
    from: [i32; 3],
    walks: &dyn Fn([i32; 3], [i32; 3]) -> bool,
    goal: [i32; 3],
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
    if kept(v.seat) {
        return Err(RideMiss::KeptOut);
    }
    let Some(board) = world
        .boarding_cells(v)
        .into_iter()
        .find(|c| !kept(*c) && walks(from, *c))
    else {
        return Err(RideMiss::NoBoard);
    };
    let tree = world.medium_tree_avoiding(v, &kept);
    // Leaving cell → the shortest ride that reaches it.
    let mut best: BTreeMap<[i32; 3], (usize, [i32; 3])> = BTreeMap::new();
    let mut longest = 0usize;
    let mut rides: Vec<(usize, [i32; 3])> = Vec::new();
    for m in tree.keys() {
        let len = World::medium_path(&tree, *m).map_or(0, |p| p.len());
        if len >= 2 {
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
    for (len, m) in rides {
        if v.kind == VehicleKind::Minecart && len != longest {
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
            let medium = World::medium_path(&tree, m).ok_or(RideMiss::NoMedium)?;
            return Ok(RideTake {
                vehicle: vi,
                board,
                medium,
                leave,
            });
        }
    }
    Err(RideMiss::NoLeave)
}

/// The seats every vehicle a ride leg's party can board carries
/// (`DW1099`'s last arm): the seats of every vehicle with a boarding cell the
/// party walks to from `from` and whose ride reaches `goal`.
pub fn boardable_seats(
    world: &World,
    vehicles: &[VehiclePlan],
    from: [i32; 3],
    walks: &dyn Fn([i32; 3], [i32; 3]) -> bool,
    goal: [i32; 3],
) -> u32 {
    (0..vehicles.len())
        .filter(|&i| ride_of(world, i, &vehicles[i], from, walks, goal).is_ok())
        .map(|i| vehicles[i].seats)
        .sum()
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
            ride_of(&w, 0, &b, from, &|_, _| false, goal),
            Err(RideMiss::NoBoard)
        );
        // Boarded, and no leaving cell walks on to the goal.
        assert_eq!(
            ride_of(&w, 0, &b, from, &|a, _| a == from, goal),
            Err(RideMiss::NoLeave)
        );
        let ride = ride_of(&w, 0, &b, from, &|_, _| true, goal).expect("a ride");
        assert_eq!(ride.medium[0], b.seat);
        assert!(ride.leave[0] >= 8, "left on the far shore: {ride:?}");
        // Two boats at one dock seat four; one seats two.
        let two = [b.clone(), boat([1, 1, 2], b.bounds)];
        assert_eq!(boardable_seats(&w, &two, from, &|_, _| true, goal), 4);
        assert_eq!(boardable_seats(&w, &two[..1], from, &|_, _| true, goal), 2);
    }

    #[test]
    fn the_medium_is_cut_at_bounds() {
        let w = pond(8);
        let all = w.medium_tree(&boat([3, 1, 0], ([-10, -5, -5], [20, 5, 5])));
        let cut = w.medium_tree(&boat([3, 1, 0], ([0, -5, -5], [4, 5, 5])));
        assert!(cut.len() < all.len() && !cut.is_empty());
    }
}
