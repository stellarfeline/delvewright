//! The router: the A* search over a [`World`], the critical path's visited
//! positions, the walked legs it proves and exports ([`LegRoute`]), and the
//! quest configurations the path passes ([`Configuration`]).
//!
//! The runtime-written region state a leg is routed under is [`region`]; the
//! leg judgement and the link-aware routing of a whole path is [`leg`].

pub(in crate::compiler::nav) mod leg;
pub(in crate::compiler::nav) mod region;
pub use leg::*;
pub(crate) use region::{LaidWrite, lay_writes};

/// A laid write with the step it fires at ([`Configuration::laid_writes`]).
pub type FiredWrite = (([i32; 3], [i32; 3]), Option<String>, usize);
pub use region::*;

use crate::compiler::failure::Failure;
use crate::compiler::nav::walkway::WalkWays;
use crate::compiler::nav::world::body::{
    AFLOAT_CLIMB_OUT_CELLS, Footprint, SNAP_RADIUS, STEP_COST_16, step_cost_16,
};
use crate::compiler::nav::world::{Liveness, World};
use crate::compiler::nav::{DW_WAYPOINT_NOT_STANDABLE, MovePlan};
use crate::compiler::plan::{Plan, RegionEvents, Step};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

impl World {
    /// The union of every cell on a critical-path walked leg (the A* paths
    /// [`check_critical_path`] validates) plus every `move-npc` waypoint cell — the
    /// "required nav path cells" the relight pass must never occupy or obstruct
    /// (spec-0010), and the cell set the lethal-trap proof calls "forced".
    ///
    /// Routed over the **causally-sealed** per-leg world, exactly like
    /// [`check_critical_path`]. Before that fix this walked the base
    /// open world while completability was proven under seals, so the two
    /// disagreed about which cells the player is actually forced across: a leg the
    /// player can only walk as a detour *because* a `close-gate` shut the direct
    /// route was routed through the (still open) gate here, and the trap proof
    /// then declared a lethal plate on the detour "avoidable" — a provable death
    /// the build shipped green.
    pub fn required_path_cells(&self, plan: &Plan, moves: &[MovePlan]) -> BTreeSet<[i32; 3]> {
        let mut cells: BTreeSet<[i32; 3]> = BTreeSet::new();
        // Critical-path legs.
        for leg in self.walked_legs(plan) {
            cells.extend(leg.cells);
        }
        // move-npc waypoint cells (floored).
        for m in moves {
            for w in &m.waypoints {
                cells.insert([
                    w[0].floor() as i32,
                    w[1].floor() as i32,
                    w[2].floor() as i32,
                ]);
            }
        }
        cells
    }

    /// The proven A* cell route for every **walked** critical-path leg (transport
    /// hops skipped), each routed over that leg's causally-sealed world — the one
    /// leg model shared by [`check_critical_path`] (the completability proof),
    /// [`World::required_path_cells`] (relight + the lethal-trap forced-cell set)
    /// and [`critical_path_routes`] (the exported harness waypoints).
    ///
    /// They are unified on purpose. Were the proof to run under `close-gate`
    /// seals while the trap analysis and the waypoint export ran over the open
    /// world, the compiler could export a bot route through a gate the campaign
    /// had already sealed, and could call a trap on a forced detour
    /// "avoidable". A leg that
    /// fails to snap or route is omitted — it cannot occur once
    /// [`check_critical_path`] has passed, and before that the DW0311 error is the
    /// diagnostic that matters.
    fn walked_legs(&self, plan: &Plan) -> Vec<LegRoute> {
        self.walked_legs_sealed(plan)
            .into_iter()
            .map(|(leg, _)| leg)
            .collect()
    }

    /// [`World::walked_legs`], each leg paired with the gate cells sealed while the
    /// player walks it ([`World::walked_leg_region_state`]). The trap proof needs the seal itself, not
    /// just the route: a disarm affordance is only genuinely reachable "before the
    /// trap" if it is reachable under the gate state in force at that point.
    pub(in crate::compiler::nav) fn walked_legs_sealed(
        &self,
        plan: &Plan,
    ) -> Vec<(LegRoute, BTreeSet<[i32; 3]>)> {
        crate::compiler::nav::shot::fix_stances(plan, self);
        route_walked_legs(
            self,
            &critical_positions(plan),
            &plan.region_events,
            &|g, s| plan.gate_fired_before(g, s),
        )
    }

    /// A* over standable cells from `start` to `goal`, returning the cell path
    /// (inclusive of both ends) or `None` if unreachable. Deterministic: the
    /// frontier is ordered by `(f, g, cell)` and neighbours expand in a fixed
    /// order.
    ///
    /// **Public** for the same reason [`World::neighbors`] is: the pacing
    /// measurement (`DW0822`'s second call site) is the length of the route a
    /// body really walks along the layout graph's own critical path, and a
    /// second router would measure a second world.
    pub fn find_path(&self, start: [i32; 3], goal: [i32; 3]) -> Option<Vec<[i32; 3]>> {
        self.find_path_fp(start, goal, &Footprint::player())
    }

    /// Footprint-aware A* (spec-0014) over the **terrain-shaped** step cost
    /// ([`step_cost_16`]) — a wider/taller footprint additionally prunes cells the
    /// puppet cannot occupy. Deterministic: frontier ordered by `(f, g, cell)`,
    /// fixed neighbour order, integer costs only.
    ///
    /// The heuristic is horizontal Manhattan distance scaled by [`STEP_COST_16`],
    /// the cost of a perfectly flat step. Since no step is ever cheaper than that,
    /// `h` stays **admissible and consistent**, so A* still returns a true
    /// minimum-cost path and never needs to reopen a closed node — the cost change
    /// is a change of *preference among valid routes*, never of which routes exist.
    pub(in crate::compiler::nav) fn find_path_fp(
        &self,
        start: [i32; 3],
        goal: [i32; 3],
        fp: &Footprint,
    ) -> Option<Vec<[i32; 3]>> {
        self.find_path_avoiding(start, goal, fp, &|_| false)
    }

    /// [`World::find_path_fp`] over the same walk graph with the cells `avoid`
    /// names withheld — the one router, asked which walks exist that never
    /// enter a set of cells. `avoid` holding `start` or `goal` finds nothing.
    /// The threshold proof (`DW1044`) asks it for a walk round a trigger's
    /// firing volume; it reads reachability here and nowhere else, so it moves
    /// with every other consumer of [`World::neighbors_fp`].
    pub(in crate::compiler::nav) fn find_path_avoiding(
        &self,
        start: [i32; 3],
        goal: [i32; 3],
        fp: &Footprint,
        avoid: &dyn Fn([i32; 3]) -> bool,
    ) -> Option<Vec<[i32; 3]>> {
        self.search(start, goal, fp, avoid, &|_| 1, false, false)
    }

    /// **The route leg's walk** (spec-0134 §10): [`World::find_path`] with the
    /// water moves added — into a connected body of water, through it in every
    /// direction, and out over the climb-out band — each water step priced at
    /// the swim rate. Every proof of where the **party** walks asks this — the
    /// critical path's legs and their export, a checkpoint's way on
    /// (`DW0315`), the respawn walk, a shortcut's long route (`DW0373`/`DW0374`)
    /// and a timed gate's disarm (`DW0393`); a body that is not a
    /// player (an NPC, an actor, a wave's mob) keeps the dry walk.
    ///
    /// **And the hop** (spec-0113 §4.5): where that walk does not reach the
    /// goal, the same search again with the jumps across a gap its frontier
    /// meets that a body on cue makes ([`World::hops_fp`], proven by the
    /// declared jump's own window) — so a leg that walks is routed exactly as
    /// before, and a leg a hop carries is one the reach graph's simulator
    /// proves a body makes, never one it merely could.
    pub fn find_route(&self, start: [i32; 3], goal: [i32; 3]) -> Option<Vec<[i32; 3]>> {
        let fp = Footprint::player();
        self.search(start, goal, &fp, &|_| false, &|_| 1, true, false)
            .or_else(|| self.search(start, goal, &fp, &|_| false, &|_| 1, true, true))
    }

    /// A player body afloat or submerged with its feet in `c`: water at the
    /// feet (never lava, never a lethal volume) and water or air at the head.
    pub fn swims_in(&self, c: [i32; 3]) -> bool {
        let head = [c[0], c[1] + 1, c[2]];
        self.is_water(c) && (self.is_water(head) || !self.is_occupied(head))
    }

    /// A body in `c` with water over its head too: a submerged cell, the run
    /// of which a leg's breath debt is printed against.
    pub fn submerged(&self, c: [i32; 3]) -> bool {
        self.is_water(c) && self.is_water([c[0], c[1] + 1, c[2]])
    }

    /// The water moves out of `c` (spec-0134 §10). From a standable cell: into
    /// an adjacent water cell at or under its level. From a water cell: to
    /// every adjacent water cell of the body (a swimmer rises and dives), and
    /// out onto a standable cell beside it within the climb-out band the
    /// stranding proof uses (`AFLOAT_CLIMB_OUT_CELLS`), with headroom over the
    /// water for the lift.
    fn swim_moves(&self, c: [i32; 3], fp: &Footprint) -> Vec<[i32; 3]> {
        const HORIZ: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
        let mut out = Vec::new();
        if self.swims_in(c) {
            for d in [
                [-1, 0, 0],
                [1, 0, 0],
                [0, 0, -1],
                [0, 0, 1],
                [0, -1, 0],
                [0, 1, 0],
            ] {
                let n = [c[0] + d[0], c[1] + d[1], c[2] + d[2]];
                if self.swims_in(n) {
                    out.push(n);
                }
            }
            for (dx, dz) in HORIZ {
                for rise in 0..=AFLOAT_CLIMB_OUT_CELLS {
                    let up = [c[0] + dx, c[1] + rise, c[2] + dz];
                    let headroom = (c[1] + 1..=c[1] + rise + 1).all(|y| {
                        !self.is_occupied([c[0], y, c[2]]) || self.is_water([c[0], y, c[2]])
                    });
                    if headroom && self.standable_fp(up, fp) {
                        out.push(up);
                        break;
                    }
                }
            }
        } else if self.standable_fp(c, fp) {
            for (dx, dz) in HORIZ {
                for dy in [0, -1] {
                    let n = [c[0] + dx, c[1] + dy, c[2] + dz];
                    if self.swims_in(n) {
                        out.push(n);
                        break;
                    }
                }
            }
        }
        out
    }

    /// **A staged walk's route**: [`World::find_path_fp`] over the same walk
    /// graph, each step charged the toll `ways` sets on the cell it enters
    /// ([`WalkWays::toll`]) — so a walk keeps to the plan's declared ways
    /// wherever they join its ends, and steps off them only to reach or leave
    /// one, or where none joins. The walk graph is the router's, so every walk
    /// this returns is one [`World::find_path_fp`] proves exists; only the
    /// preference among them moves. With no way declared every toll is one and
    /// the route is [`World::find_path_fp`]'s, byte for byte.
    pub(in crate::compiler::nav) fn find_walk_fp(
        &self,
        start: [i32; 3],
        goal: [i32; 3],
        fp: &Footprint,
        ways: &WalkWays<'_>,
    ) -> Option<Vec<[i32; 3]>> {
        self.search(start, goal, fp, &|_| false, &|c| ways.toll(c), false, false)
    }

    /// The one A* every route above asks: `avoid` withholds cells, `toll`
    /// multiplies the step cost of entering a cell. A toll is never under one,
    /// so the flat-step heuristic stays admissible and consistent. `swim` adds
    /// the water moves, `hop` the hops ([`World::hops_fp`]), each priced at
    /// one flat step more than the columns it crosses, so the heuristic stays
    /// admissible over it too.
    #[allow(clippy::too_many_arguments)]
    fn search(
        &self,
        start: [i32; 3],
        goal: [i32; 3],
        fp: &Footprint,
        avoid: &dyn Fn([i32; 3]) -> bool,
        toll: &dyn Fn([i32; 3]) -> u32,
        swim: bool,
        hop: bool,
    ) -> Option<Vec<[i32; 3]>> {
        if avoid(start) || avoid(goal) {
            return None;
        }
        if start == goal {
            return self.standable_fp(start, fp).then(|| vec![start]);
        }
        if !self.standable_fp(start, fp) || !self.standable_fp(goal, fp) {
            return None;
        }
        let h =
            |c: [i32; 3]| ((c[0] - goal[0]).abs() + (c[2] - goal[2]).abs()) as u32 * STEP_COST_16;
        let mut g_score: BTreeMap<[i32; 3], u32> = BTreeMap::new();
        let mut came_from: BTreeMap<[i32; 3], [i32; 3]> = BTreeMap::new();
        let mut open: BinaryHeap<Reverse<(u32, u32, [i32; 3])>> = BinaryHeap::new();
        g_score.insert(start, 0);
        open.push(Reverse((h(start), 0, start)));
        while let Some(Reverse((_f, g, cur))) = open.pop() {
            if cur == goal {
                let mut path = vec![cur];
                let mut node = cur;
                while let Some(&prev) = came_from.get(&node) {
                    path.push(prev);
                    node = prev;
                }
                path.reverse();
                return Some(path);
            }
            // Skip stale heap entries (a cheaper route was already recorded).
            if g > *g_score.get(&cur).unwrap_or(&u32::MAX) {
                continue;
            }
            let wet = swim && self.swims_in(cur);
            let mut next = if wet {
                Vec::new()
            } else {
                self.neighbors_fp(cur, fp)
            };
            if swim {
                for n in self.swim_moves(cur, fp) {
                    if !next.contains(&n) {
                        next.push(n);
                    }
                }
            }
            let hops = if hop && !wet {
                self.hops_fp(cur, fp)
            } else {
                Vec::new()
            };
            for n in &hops {
                if !next.contains(n) {
                    next.push(*n);
                }
            }
            let here = if wet { 0 } else { self.feet_16_fp(cur, fp) };
            for n in next {
                if avoid(n) {
                    continue;
                }
                // spec-0134 §10: a step into or out of water is priced at the
                // swim rate; the flat-step heuristic stays admissible because
                // the swim is slower than the walk.
                let cost = if hops.contains(&n) {
                    let across = ((n[0] - cur[0]).abs() + (n[2] - cur[2]).abs()) as u32;
                    STEP_COST_16 * (across + 1)
                } else if wet || (swim && self.swims_in(n)) {
                    swim_step_cost()
                } else {
                    step_cost_16(here, self.feet_16_fp(n, fp))
                };
                let tentative = g + cost * toll(n).max(1);
                if tentative < *g_score.get(&n).unwrap_or(&u32::MAX) {
                    came_from.insert(n, cur);
                    g_score.insert(n, tentative);
                    open.push(Reverse((tentative + h(n), tentative, n)));
                }
            }
        }
        None
    }
}

/// The A* cost of one swum block (spec-0134 §10): the flat step's cost scaled
/// by the measured swim rate over the walk's (`pacing.ticks-per-block-swimming`
/// over `walk.ticks-per-block`), rounded up — never under the flat step, so the
/// route's heuristic stays admissible.
pub(in crate::compiler::nav) fn swim_step_cost() -> u32 {
    static COST: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *COST.get_or_init(|| {
        let swim = delvewright_dsl::metrics::Metrics::table()
            .swim_ticks_per_block(&mut delvewright_dsl::metrics::Reads::new())
            .expect("the metrics table defines the swim rate");
        let ratio = swim / delvewright_dsl::metrics::walk_ticks_per_block();
        ((f64::from(STEP_COST_16) * ratio).ceil() as u32).max(STEP_COST_16)
    })
}

/// The player-visited critical-path positions in order, each tagged with whether
/// the player was teleported here by an inter-area transport on the preceding
/// step (a ride, not a walk). `select-class` / `assert-complete` steps carry no
/// position and are skipped.
/// A player-visited critical-path position, with the metadata walked-leg routing
/// needs. Replaces the bare `([i32;3], bool)` tuple so a talk-to target — whose
/// anchor cell is the NPC's own occupied cell — can be endpoint-snapped correctly.
#[derive(Debug, Clone, Copy)]
pub(in crate::compiler::nav) struct VisitedPos {
    /// The raw visited anchor cell (an NPC stand, altar, chest, wave marker, …).
    pub(in crate::compiler::nav) pos: [i32; 3],
    /// The player rides an inter-area transport INTO this position, so the move
    /// here is a teleport, not a walk to validate/export.
    pub(in crate::compiler::nav) transport_before: bool,
    /// This position is a talk-to NPC anchor: the player stands *within interaction
    /// range beside* the NPC, never on the mannequin-occupied anchor cell, so the
    /// goal snap must exclude that cell.
    pub(in crate::compiler::nav) talk_to: bool,
    /// The originating `critical_path` step index (v0.6): lets the checkpoint /
    /// stealth proofs select the positions at or after a firing step.
    pub(in crate::compiler::nav) src_step: usize,
    /// The party arrives here by a **link** (spec-0083) — `transport_before` is
    /// then also set, and the carry is counted as a link's rather than a
    /// crossing's.
    pub(in crate::compiler::nav) by_link: bool,
    /// The party arrives here by a **loop**'s exercise move (spec-0086 §5.2) —
    /// `transport_before` is then also set, and the carry is counted as a
    /// loop's.
    pub(in crate::compiler::nav) by_loop: bool,
}

/// How many critical-path **legs** the completability proof routes — consecutive
/// visited pairs, minus the inter-area teleport hops it skips. The binding count
/// `validation/lethal-gate.json` reports for `DW0510`: a lethal volume proven over
/// zero legs is a vacuous green, and this is what makes that legible.
pub fn critical_leg_count(plan: &Plan) -> usize {
    critical_positions(plan)
        .windows(2)
        .filter(|pair| !pair[1].transport_before)
        .count()
}

pub(in crate::compiler::nav) fn critical_positions(plan: &Plan) -> Vec<VisitedPos> {
    positions_of(
        plan.campaign_start().map(|(_, pos)| pos),
        &plan.critical_path,
        &plan.critical_path_transport,
        plan.shot_stances.get(),
    )
}

/// [`critical_positions`] over an arbitrary exported step list — the shared core,
/// split out so a spec-0025 **branch path** (a different sequence of
/// the same step shapes, with its own transport markers) yields its own visited
/// positions in its own step space. `src_step` indices are indices into `steps`.
///
/// `start` is the cell the party begins the delve standing on
/// ([`Plan::campaign_start`]), and it is the **first** visited position: the
/// party's opening move is a leg like any other, and leaving it out is what let
/// a campaign whose first objective stood across a void compile clean and strand
/// the bot. It is never arrived at by transport — a crossing rides on an
/// objective completion and at the spawn there is none, which
/// `plan::DW_SPAWN_LEG_CROSSES` refuses before any of this runs — so the leg out
/// of it is always a walk for `DW0311` to prove. It carries `src_step: 0`, the
/// class-select step, which is what every `src_step > fire_step` filter already
/// treats as preceding everything.
///
/// A `shot` step stands where it is loosed from: its stance in `shots`
/// ([`crate::compiler::nav::shot::fix_stances`]), the cell the export hands
/// the bot as its `stand`, never the target it hits.
pub(in crate::compiler::nav) fn positions_of(
    start: Option<[i32; 3]>,
    steps: &[Step],
    transports: &[Option<[i32; 3]>],
    shots: Option<&BTreeMap<String, [i32; 3]>>,
) -> Vec<VisitedPos> {
    let mut out = Vec::new();
    if let Some(pos) = start {
        out.push(VisitedPos {
            pos,
            transport_before: false,
            talk_to: false,
            src_step: 0,
            by_link: false,
            by_loop: false,
        });
    }
    let mut transport_pending = false;
    for (i, step) in steps.iter().enumerate() {
        // A carried step: a link's trigger step (spec-0083 §3.2) or a loop's
        // exercise step (spec-0086 §5.2). The leg into it ends where the party
        // stands to be carried, and the party goes on from where it is put down.
        // The carry between the two is marked like a crossing, so every reader of
        // this enumeration skips it as a ride; one branch for both carries, so a
        // link and a loop look the same to every proof that reads positions
        // (spec-0086 §5.4).
        let carry = match step {
            Step::Loop { pos, transport, .. } | Step::Ride { pos, transport, .. } => {
                Some((*pos, Some(*transport), false, true))
            }
            _ => step
                .stand()
                .map(|stand| (stand, transports.get(i).copied().flatten(), true, false)),
        };
        if let Some((stand, to, by_link, by_loop)) = carry {
            out.push(VisitedPos {
                pos: stand,
                transport_before: transport_pending,
                talk_to: false,
                src_step: i,
                by_link: false,
                by_loop: false,
            });
            transport_pending = false;
            if let Some(to) = to {
                out.push(VisitedPos {
                    pos: to,
                    transport_before: true,
                    talk_to: false,
                    src_step: i,
                    by_link,
                    by_loop,
                });
            }
            continue;
        }
        // A `trigger` step stands somewhere like an objective does: the party walks
        // to what it strikes, so the leg to it is a leg the proof owes — a shot
        // to the stance it is loosed from.
        let loosed = match step {
            Step::Trigger {
                on: "shot",
                trigger_id,
                ..
            } => shots.and_then(|m| m.get(trigger_id.as_str())).copied(),
            _ => None,
        };
        if let Some(pos) = loosed.or_else(|| step.pos()) {
            out.push(VisitedPos {
                pos,
                transport_before: transport_pending,
                talk_to: matches!(step, Step::TalkTo { .. }),
                src_step: i,
                by_link: false,
                by_loop: false,
            });
            transport_pending = false;
        }
        // A transport marker on step `i` teleports the player when that step's
        // objective completes — i.e. before the *next* visited position is reached,
        // so the move INTO that next position is a ride, not a walk to validate.
        if transports.get(i).and_then(|t| *t).is_some() {
            transport_pending = true;
        }
    }
    out
}

/// Whether the campaign has at least one consecutive pair of player-visited
/// critical-path positions with no inter-area transport between them — a leg the
/// player must walk, hence one DW0311 must validate.
pub(in crate::compiler::nav) fn has_walkable_critical_leg(plan: &Plan) -> bool {
    critical_positions(plan)
        .windows(2)
        .any(|w| !w[1].transport_before)
}

/// Validate that every consecutive pair of player-visited critical-path anchors is
/// connected by a walkable A* path over the assembled geometry (unless the player
/// rides an inter-area transport between them). This is the compile-time counterpart
/// to the runtime critical-path bot: it makes an unwalkable assembled seam — a
/// prefab whose regenerated geometry wedged a doorway shut or opened a void gap — a
/// build failure ([`DW_CRITICAL_UNROUTABLE`], `DW0311`) instead of a bot surprise.
///
/// Endpoints are snapped to the nearest standable floor cell (an anchor often marks
/// a solid affordance — an altar, a wave marker, an NPC stand — the player walks up
/// to, not into), exactly as `move-npc` planning does.
pub fn check_critical_path(plan: &Plan, world: &World) -> Result<(), Failure> {
    check_critical_path_bound(plan, world).1
}

/// [`check_critical_path`] with its binding (spec-0083 §5), returned beside the
/// verdict so the build can print it whichever way the proof went.
pub fn check_critical_path_bound(
    plan: &Plan,
    world: &World,
) -> (RouteBinding, Result<(), Failure>) {
    crate::compiler::nav::shot::fix_stances(plan, world);
    let (b, r) = route_with_links(
        world,
        &critical_positions(plan),
        &plan.region_events,
        &|g, s| plan.gate_fired_before(g, s),
        &Carries::of_plan(plan),
    );
    (b, r.map(|_| ()))
}

/// **Which links the default path takes** (spec-0083 §3.4): the walk proof run
/// over the plan's path, handing back each leg a walk could not cross and the
/// links that carry it, keyed by the plan's own step indices — what
/// [`Plan::relinked`] splices into the path. Empty for every campaign whose legs
/// all walk.
pub fn take_links(plan: &Plan, world: &World) -> Result<crate::compiler::plan::LinkTakes, Failure> {
    // A shot's leg ends at its stance on this proof as on every other
    // (`shot::fix_stances`): the links and rides a campaign declares are taken
    // over the same legs the walk proof walks.
    crate::compiler::nav::shot::fix_stances(plan, world);
    route_with_links(
        world,
        &critical_positions(plan),
        &plan.region_events,
        &|g, s| plan.gate_fired_before(g, s),
        &Carries::of_plan(plan),
    )
    .1
}

/// **The plan every reader of the path reads** (spec-0083 §3.4): `plan` with
/// the links the route proof takes over `world` spliced in, or `None` when it
/// takes none and `plan` is already that path. The build, the review frames
/// and the blocking chart all go through here, so a `pov/…` shot, a corridor
/// and the proof are taken over one path. A leg nothing carries is not refused
/// here: the walk proof refuses it in its own place.
pub fn with_links_taken<'a>(
    plan: &Plan<'a>,
    prefabs: &crate::compiler::registry::PrefabRegistry,
    world: &World,
) -> Result<Option<Plan<'a>>, Failure> {
    if plan.links.is_empty() && !plan.region_events.has_keep_out() && plan.vehicles.is_empty() {
        return Ok(None);
    }
    let takes = take_links(plan, world).unwrap_or_default();
    if takes.is_empty() {
        return Ok(None);
    }
    let relinked = plan.relinked(prefabs, takes).map_err(|e| e.failure)?;
    // One stance per shot, whichever plan of the lineage asks: the relinked
    // plan is this plan with carries spliced in, and its shots are loosed from
    // where this plan's are.
    if let Some(m) = plan.shot_stances.get() {
        let _ = relinked.shot_stances.set(m.clone());
    }
    // The relinked path is the proof's own decision: a second pass over it
    // must take nothing more, or the path and the proof disagree.
    if !take_links(&relinked, world)?.is_empty() {
        return Err(Failure {
            code: crate::compiler::plan::DW_BUILD,
            message: "internal invariant violation: the route proof asked for a link on a \
                      path that already carries every link it took (spec-0083 §3.4). This \
                      is a compiler bug; stop and escalate"
                .to_string(),
        });
    }
    Ok(Some(relinked))
}

/// [`take_links`] over one branch's path (spec-0083 §6): the same proof over
/// the branch's own steps, live links and gate model, keyed by the branch's
/// own step indices — what [`Plan::branch_critical_path_linked`] splices.
pub fn take_branch_links(
    plan: &Plan,
    world: &World,
    start: Option<[i32; 3]>,
    cp: &crate::compiler::plan::CriticalPath,
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<crate::compiler::plan::LinkTakes, Failure> {
    crate::compiler::nav::shot::fix_stances(plan, world);
    route_with_links(
        world,
        &positions_of(
            start,
            &cp.steps,
            &cp.transport_by_step,
            plan.shot_stances.get(),
        ),
        region_events,
        ancestor,
        &Carries {
            links: &plan.links,
            gathers: &plan.gathers,
            live: &cp.live_links_by_step,
            steps: &cp.steps,
            vehicles: &plan.vehicles,
            declared: &plan.declared_rides,
            aboard: &plan.rides_aboard,
        },
    )
    .1
}

/// **Every cell the party's own forced walk crosses**, attributed to the
/// critical-path step it is walking to (spec-0044 §6).
///
/// This is [`check_critical_path`]'s route, kept rather than discarded: the same
/// legs, over the same per-leg region state, snapped the same way. A step's entry
/// holds the routed leg INTO it plus its own visited cell, so "the party must
/// stand here by step `s`" is a fact about the same walk `DW0311` proved.
///
/// A leg the player rides rather than walks contributes only its destination
/// cell, and an unroutable leg contributes only its endpoints — `DW0311` owns
/// that failure, and a proof that invented cells for it would be reading a walk
/// the party cannot make.
pub(crate) fn critical_route_cells(plan: &Plan, world: &World) -> Vec<(usize, Vec<[i32; 3]>)> {
    let positions = critical_positions(plan);
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let mut out: Vec<(usize, Vec<[i32; 3]>)> = Vec::new();
    if let Some(first) = positions.first() {
        out.push((first.src_step, vec![first.pos]));
    }
    for pair in positions.windows(2) {
        let mut cells = vec![pair[1].pos];
        if !pair[1].transport_before {
            let st = world.walked_leg_region_state(
                &plan.region_events,
                &ancestor,
                pair[0].src_step,
                pair[1].src_step,
            );
            let leg_owned;
            let leg: &World = if st.is_empty() {
                world
            } else {
                leg_owned = world.with_region_state(&st);
                &leg_owned
            };
            if let (Some(a), Some(b)) = (
                leg.snap_endpoint(pair[0].pos, pair[0].talk_to),
                leg.snap_endpoint(pair[1].pos, pair[1].talk_to),
            ) && let Some(path) = leg.find_route(a, b)
            {
                cells.extend(path);
            }
        }
        cells.sort_unstable();
        cells.dedup();
        out.push((pair[1].src_step, cells));
    }
    out
}

/// This world as the quest configuration stands when critical step `step` is
/// next — its gates as `DW0921` judges that configuration — or `None` when that
/// configuration writes nothing, so the caller keeps the world it has.
///
/// Public for `DW0924`, which asks where the party can walk while a fight is the
/// beat the story waits on: a gate a later beat opens is shut until the fight
/// is won, so it is no way to the fight.
pub fn world_while_next(plan: &Plan, world: &World, step: usize) -> Option<World> {
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let st = world.region_state_at(&plan.region_events, step, &ancestor);
    (!st.is_empty()).then(|| world.with_region_state(&st))
}

/// One quest configuration the critical path passes (spec-0088 §5): the first
/// critical step that arrives under it, its region state, and every staged
/// lethal volume's [`Liveness`] there.
pub struct Configuration {
    /// The first critical-path step whose arrival is judged under it.
    pub step: usize,
    state: RegionState,
    /// One entry per [`World::staged_volumes`], in declaration order.
    pub live: Vec<Liveness>,
}

impl Configuration {
    /// Whether two configurations hold one region state.
    pub fn same_state(&self, other: &Configuration) -> bool {
        self.state == other.state
    }

    /// This configuration's world: `base` with every runtime write it credits,
    /// staged volumes included, or `None` when it writes nothing.
    pub fn world(&self, base: &World) -> Option<World> {
        (!self.state.is_empty()).then(|| base.with_region_state(&self.state))
    }

    /// This configuration's bytes ([`RegionState::blocks_over`]).
    pub fn blocks(
        &self,
        base: &crate::compiler::blockstate::BlockMap,
    ) -> crate::compiler::blockstate::BlockMap {
        self.state.blocks_over(base)
    }

    /// How many cells this configuration's bytes ([`Configuration::blocks`]
    /// over the same base) hold differently from `load` — counted over the
    /// cells its laid writes reach, which are the only cells
    /// [`RegionState::blocks_over`] can move (spec-0089 §7's `cells moved from
    /// load`). Every forced write `load` lays is laid here too or overridden
    /// by a later write on its region, so no moved cell lies outside.
    pub fn moved_from(&self, load: &crate::compiler::blockstate::BlockMap) -> usize {
        self.laid()
            .iter()
            .filter(|(c, b)| load.get(c).map(|s| s.as_str()) != b.as_deref())
            .count()
    }

    /// **The cells this configuration's laid writes reach**, each with the block
    /// the last write on it lays (`None`: air) — the only cells
    /// [`Configuration::blocks`] can move from its base.
    pub fn laid(&self) -> BTreeMap<[i32; 3], Option<String>> {
        let mut over: BTreeMap<[i32; 3], Option<String>> = BTreeMap::new();
        for ((lo, hi), block) in &self.state.laid {
            match block {
                // Cell by cell from the write's block (`region_fill`).
                Some(b) => {
                    for (c, s) in crate::compiler::region_fill::cells((*lo, *hi), b) {
                        over.insert(c, Some(s.into_owned()));
                    }
                }
                None => {
                    for c in crate::compiler::assembled::region_cells(*lo, *hi) {
                        over.insert(c, None);
                    }
                }
            }
        }
        over
    }

    /// **The regions this configuration's laid writes reach**, in the order
    /// laid — whatever block each lays, air included: a write that lays air
    /// clears what the level generator put in the cell, so a written world
    /// asks this, never only the block map, which cannot tell a cleared cell
    /// from one nothing wrote.
    pub fn laid_regions(&self) -> Vec<([i32; 3], [i32; 3])> {
        self.state.laid.iter().map(|(r, _)| *r).collect()
    }

    /// **Every write this configuration lays, with the step it fires at**, in
    /// the order laid: the region, the block it lays (`None`: air) and the
    /// critical-path step whose completion fires it (`0` for a world-load
    /// write). What a picture orders against the placements ledger's rows, so
    /// a cell the setup writes and a later beat rewrites holds the beat's
    /// block.
    pub fn laid_writes(&self) -> Vec<FiredWrite> {
        self.state
            .laid
            .iter()
            .zip(self.state.laid_steps.0.iter())
            .map(|((r, b), s)| (*r, b.clone(), *s))
            .collect()
    }

    /// How many regions an **unforced** write holds here — writes a beat
    /// nobody has to play lays, which [`RegionState::blocks_over`] does not lay
    /// (spec-0089 §7's `unforced write(s) not laid`).
    pub fn unforced_writes(&self) -> usize {
        self.state.unforced_regions.len()
    }
}

/// **The configuration a path holds on arrival at step `arrival`** — the state
/// [`World::region_state_at`] gives that arrival over `events` under
/// `ancestor`, the one every route proof asks. `arrival` may be the path's
/// length: the end state, every step on the path preceding it.
///
/// Public for spec-0089: a showcase camera taken after step `i` stands in the
/// configuration arriving at `i + 1`, and the plan's POV shots in the one
/// arriving at their leg.
pub fn configuration_at(
    world: &World,
    events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
    arrival: usize,
) -> Configuration {
    Configuration {
        step: arrival,
        live: world.staged_liveness(events, arrival, ancestor),
        state: world.region_state_at(events, arrival, ancestor),
    }
}

/// **The world each of `steps` is judged in** (spec-0104 §4): for every
/// arrival, the configuration [`configuration_at`] gives it — `None` where it
/// writes nothing and the base world stands — with arrivals under one region
/// state sharing one world, so a sight proof over many acts builds each
/// configuration once. Returns, per step in `steps`' order, an index into the
/// returned worlds.
pub fn worlds_at(
    world: &World,
    events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
    steps: &[usize],
) -> (Vec<Option<World>>, Vec<usize>) {
    let mut states: Vec<RegionState> = Vec::new();
    let mut worlds: Vec<Option<World>> = Vec::new();
    let mut index = Vec::with_capacity(steps.len());
    for &step in steps {
        let st = world.region_state_at(events, step, ancestor);
        let i = match states.iter().position(|s| *s == st) {
            Some(i) => i,
            None => {
                worlds.push((!st.is_empty()).then(|| world.with_region_state(&st)));
                states.push(st);
                states.len() - 1
            }
        };
        index.push(i);
    }
    (worlds, index)
}

/// **The configuration a walked leg is walked under** (spec-0112 §5 item 1):
/// the region state the route proof walks the leg `from_step → to_step` in
/// ([`World::walked_leg_region_state`]) — every write the arrival's ancestors,
/// the start step and the start's ancestors fire — so a frame standing on the
/// leg sees the doors the walk passed through as the walk found them.
pub fn leg_configuration(
    world: &World,
    events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
    from_step: usize,
    to_step: usize,
) -> Configuration {
    let fired = region::leg_fired(ancestor, from_step, to_step);
    Configuration {
        step: to_step,
        live: world.staged_liveness(events, to_step, &fired),
        state: world.walked_leg_region_state(events, ancestor, from_step, to_step),
    }
}

/// The "has the party fired step `g`" predicate of a walked leg
/// ([`leg_configuration`]'s), in the `(fire_step, arrival)` shape the
/// placements ledger is asked in ([`crate::compiler::placed::rows_under`]).
pub fn leg_fired<'a>(
    ancestor: &'a dyn Fn(usize, usize) -> bool,
    from_step: usize,
    to_step: usize,
) -> impl Fn(usize, usize) -> bool + 'a {
    region::leg_fired(ancestor, from_step, to_step)
}

/// **The quest configurations the critical path passes**, in path order — one per
/// distinct region state, keyed by the first arrival under it — with `per_step`
/// mapping every critical step to its configuration's index (spec-0088 §5). The
/// enumeration `DW0891` judges a staged volume over, built from the same
/// [`World::region_state_at`] every route proof asks.
pub fn path_configurations(plan: &Plan, world: &World) -> (Vec<Configuration>, Vec<usize>) {
    let ancestor = |g: usize, s: usize| plan.gate_fired_before(g, s);
    let mut configs: Vec<Configuration> = Vec::new();
    let per_step = configurations_along(
        world,
        &plan.region_events,
        &ancestor,
        plan.critical_path.len(),
        &mut configs,
    )
    .into_iter()
    .map(|(ci, _)| ci)
    .collect();
    (configs, per_step)
}

/// The configurations one path passes — `steps` arrivals over `events` in that
/// path's own step space, under its own ancestry — appended to `configs` where
/// they are new (a configuration is its region state, whichever path reaches
/// it). Returns, per step, the configuration's index and every staged volume's
/// [`Liveness`] at that step.
pub fn configurations_along(
    world: &World,
    events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
    steps: usize,
    configs: &mut Vec<Configuration>,
) -> Vec<(usize, Vec<Liveness>)> {
    let mut out = Vec::new();
    for step in 0..steps {
        let st = world.region_state_at(events, step, ancestor);
        let live = world.staged_liveness(events, step, ancestor);
        let idx = match configs.iter().position(|c| c.state == st) {
            Some(i) => i,
            None => {
                configs.push(Configuration {
                    step,
                    live: live.clone(),
                    state: st,
                });
                configs.len() - 1
            }
        };
        out.push((idx, live));
    }
    out
}

/// A walked critical-path leg with the full A* cell route the compiler proved
/// connects it — the export counterpart of [`check_critical_path`].
/// `from`/`to` are the raw visited anchor cells (identical to the harness
/// `critical-path.json` step positions, so the harness can key a leg by its
/// destination); `cells` is the standable-cell polyline between their snapped floor
/// endpoints, inclusive of both.
#[derive(Debug, Clone)]
pub struct LegRoute {
    /// The raw visited anchor the leg walks FROM (the previous critical position).
    pub from: [i32; 3],
    /// The raw visited anchor the leg walks TO (this critical position).
    pub to: [i32; 3],
    /// The `critical_path` step index of the position the leg walks FROM — the
    /// step the party has just completed when the leg begins.
    pub from_step: usize,
    /// The `critical_path` step index of the destination position — the objective
    /// this leg walks toward. Lets the visual-tier POV planner
    /// (`crate::compiler::render_plan`) name the served objective without re-deriving the
    /// leg selection.
    pub to_step: usize,
    /// The standable-cell A* path from the snapped `from` floor to the snapped `to`
    /// floor, inclusive of both endpoints.
    pub cells: Vec<[i32; 3]>,
    /// The cells of `cells` that are closed fence gates the player must right-click
    /// open to pass ("use-gate" edges), in path order. Exported in the
    /// waypoint metadata so the harness bot knows the leg crosses a gate (its
    /// pathfinder's `canOpenDoors` performs the adventure-legal click); always
    /// kept as thinned waypoints.
    pub use_gates: Vec<[i32; 3]>,
    /// The climbs the route takes (spec-0099), in route order, as
    /// [`World::climb_runs`] reads them off `cells` in the world the leg was
    /// proven over. Exported in the waypoint metadata so the harness bot drives
    /// the climb the compiler proved; empty for every leg that climbs nothing.
    pub climbs: Vec<crate::compiler::nav::ClimbRun>,
    /// The cells of `cells` a body swims in (spec-0134 §10), in path order —
    /// exported as the leg's `water` so the harness admits liquid on this leg
    /// and no other; empty for every dry leg.
    pub water: Vec<[i32; 3]>,
    /// The hops the route takes (spec-0113 §4.5, [`World::hops_fp`]), in
    /// route order, as `(from, to, sprint, run_up)`: jumps across a gap no
    /// declaration names, proven by the declared jump's own window
    /// ([`World::hop_sprint`]). Exported among the leg's `jumps` so the
    /// harness drives each as it drives a declared one; empty for every leg
    /// the walk carries.
    pub hops: Vec<([i32; 3], [i32; 3], bool, u32)>,
    /// The runtime-region state in force while the player walks this leg — the
    /// world the A* above actually ran in ([`World::walked_leg_region_state`]).
    ///
    /// **Private, and it is the point.** A `LegRoute` is only ever produced by
    /// [`route_walked_legs`], which is the single site that decides this value; a
    /// consumer that wants to re-judge these cells cannot supply an opinion of its
    /// own, it can only ask [`LegRoute::proven_world`]. Before the leg carried it,
    /// the state was computed, used for the route, and dropped — so the
    /// standability self-check re-judged the cells against the base world and
    /// refused every route that crossed floor the campaign lays at runtime.
    pub(in crate::compiler::nav) region_state: RegionState,
}

impl LegRoute {
    /// The world this leg's route was proven over: `world` as this leg's runtime
    /// region writes leave it. `None` when the leg has no runtime writes in force,
    /// which is every leg of every campaign that writes no region — those judge
    /// `world` itself and clone nothing.
    ///
    /// The one way to obtain a leg's world. It exists so that "which world does
    /// this route mean" has exactly one answer, held by the value that carries the
    /// route, rather than one answer per caller.
    pub(in crate::compiler::nav) fn proven_world(&self, world: &World) -> Option<World> {
        (!self.region_state.is_empty()).then(|| world.with_region_state(&self.region_state))
    }

    /// The same leg walked from somewhere else: an A* route from `from` (snapped
    /// to standable footing) to this leg's own snapped destination, over the
    /// world this leg was proven in. `None` when `from` has no footing or no
    /// route — the caller keeps the proven leg.
    ///
    /// The critical path's legs run step to step, and a rest is not a step: the
    /// party walks to the fire, rests, and sets off from the fire. The leg that
    /// follows a rest therefore starts at the fire, which is what a question
    /// about that leg (the run-back finder) has to route.
    pub(crate) fn rerouted_from(&self, world: &World, from: [i32; 3]) -> Option<Vec<[i32; 3]>> {
        let owned = self.proven_world(world);
        let w: &World = owned.as_ref().unwrap_or(world);
        let start = w.snap_standable(from, SNAP_RADIUS)?;
        let goal = *self.cells.last()?;
        w.find_route(start, goal)
    }
}

/// Compute the proven A* cell route for every WALKED critical-path leg (transport
/// hops skipped), for export as validation metadata (see the `waypoints` module).
/// Mirrors [`check_critical_path`]'s leg selection, endpoint snapping **and
/// per-leg gate seals** exactly, so an exported leg is the same route
/// the DW0311 guard proved routable — and an exported waypoint can never cross a
/// gate a `close-gate` has already shut by the time the bot walks that leg.
/// Intended to be called only after [`check_critical_path`] has succeeded; a leg
/// that fails to snap or route is silently omitted (cannot occur once the check
/// has passed).
pub fn critical_path_routes(plan: &Plan, world: &World) -> Vec<LegRoute> {
    world.walked_legs(plan)
}

/// Per-branch `DW0311` (spec-0025): prove every walked leg of ONE
/// branch's exported path is routable over the assembled geometry, under the
/// branch's own causal gate seals.
///
/// [`check_critical_path`] quantifies over the DEFAULT playthrough only; a
/// branch-divergent leg — one the fork adds or resequences — was walked by the
/// harness with no compile-time proof behind it. This is the same
/// [`route_visited`] core over the branch's own step list, with `region_events` /
/// `ancestor` in the **branch path's step space**
/// ([`Plan::branch_gate_model`]) — never the default path's indices, which
/// belong to a different sequence.
pub fn check_branch_path(
    plan: &Plan,
    world: &World,
    start: Option<[i32; 3]>,
    cp: &crate::compiler::plan::CriticalPath,
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<(), Failure> {
    crate::compiler::nav::shot::fix_stances(plan, world);
    route_with_links(
        world,
        &positions_of(
            start,
            &cp.steps,
            &cp.transport_by_step,
            plan.shot_stances.get(),
        ),
        region_events,
        ancestor,
        &Carries {
            links: &plan.links,
            gathers: &plan.gathers,
            live: &cp.live_links_by_step,
            steps: &cp.steps,
            vehicles: &plan.vehicles,
            declared: &plan.declared_rides,
            aboard: &plan.rides_aboard,
        },
    )
    .1
    .map(|_| ())
}

/// The proven A* cell routes of one branch's walked legs — the branch
/// counterpart of [`critical_path_routes`], for export as that branch's waypoint
/// artifact (`validation/branch-waypoints-<branch>.json`). Same leg
/// selection, endpoint snapping and per-leg gate seals as [`check_branch_path`];
/// call it only after that check has succeeded (a leg that fails to snap or
/// route is omitted, which cannot occur once the check has passed).
#[allow(clippy::too_many_arguments)]
pub fn branch_path_routes(
    world: &World,
    start: Option<[i32; 3]>,
    steps: &[Step],
    transports: &[Option<[i32; 3]>],
    shots: Option<&BTreeMap<String, [i32; 3]>>,
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Vec<LegRoute> {
    route_walked_legs(
        world,
        &positions_of(start, steps, transports, shots),
        region_events,
        ancestor,
    )
    .into_iter()
    .map(|(leg, _)| leg)
    .collect()
}

/// Assert every exported waypoint cell is standable in `world` — the final model the
/// routes were computed over (settled + flooded + fixtures). Returns
/// [`DW_WAYPOINT_NOT_STANDABLE`] (`DW0314`) naming the first offending cell/leg on
/// violation. This is the structural guard the water-flood model exists to make
/// enforceable: a waypoint in a flooded (or newly-walled) cell fails the build
/// loudly instead of stranding the bot at runtime.
pub fn verify_exported_routes(world: &World, routes: &[LegRoute]) -> Result<(), Failure> {
    for leg in routes {
        // The world this leg was PROVEN over, obtained from the leg rather than
        // decided here. `world` is the final assembled model (settled + flooded +
        // fixtures); the leg's own runtime region writes are laid over it, which is
        // exactly the model the A* ran in. Judging `world` bare instead is the
        // second opinion this field exists to remove: a leg the campaign lays floor
        // for is walkable when it is walked and not before, so the bare world calls
        // its cells "no floor" and refuses a route that is correct.
        let leg_world_owned = leg.proven_world(world);
        let leg_world: &World = leg_world_owned.as_ref().unwrap_or(world);
        for &cell in &leg.cells {
            // A cell a route holds a body in on a climb (spec-0099) is a cell the
            // body can be in, though it stands on nothing.
            // A water cell of a route (spec-0134 §10) holds a body afloat.
            if !leg_world.holds_body(cell)
                && !(leg.water.contains(&cell) && leg_world.swims_in(cell))
            {
                return Err(Failure {
                    code: DW_WAYPOINT_NOT_STANDABLE,
                    message: format!(
                        "critical-path waypoint export: cell {cell:?} on the leg to {to:?} is not \
                         standable in the final assembled world as this leg's runtime region \
                         writes leave it (it is solid, water-flooded, or has no floor). A proven \
                         route must not cross a cell a later pass mutated — this is the \
                         water-flow / post-nav-mutation divergence class: fix the prefab/water or \
                         the assembly, do not move the waypoint. (leg from {from:?})",
                        to = leg.to,
                        from = leg.from,
                    ),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
