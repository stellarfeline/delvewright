//! The router: the A* search over a [`World`], the critical path's visited
//! positions, the walked legs it proves and exports ([`LegRoute`]), and the
//! quest configurations the path passes ([`Configuration`]).
//!
//! The runtime-written region state a leg is routed under is [`region`]; the
//! leg judgement and the link-aware routing of a whole path is [`leg`].

pub(in crate::compiler::nav) mod leg;
pub(in crate::compiler::nav) mod region;
pub use leg::*;
pub use region::*;

use crate::compiler::failure::Failure;
use crate::compiler::nav::world::body::{Footprint, SNAP_RADIUS, STEP_COST_16, step_cost_16};
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
            let here = self.feet_16_fp(cur, fp);
            for n in self.neighbors_fp(cur, fp) {
                let tentative = g + step_cost_16(here, self.feet_16_fp(n, fp));
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
pub(in crate::compiler::nav) fn positions_of(
    start: Option<[i32; 3]>,
    steps: &[Step],
    transports: &[Option<[i32; 3]>],
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
            Step::Loop { pos, transport, .. } => Some((*pos, Some(*transport), false, true)),
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
        // to what it strikes, so the leg to it is a leg the proof owes.
        if let Some(pos) = step.pos() {
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
/// takes none and `plan` is already that path. The build, the snapshot camera
/// and the blocking chart all go through here, so a `pov/…` shot, a corridor
/// and the proof are taken over one path. A leg nothing carries is not refused
/// here: the walk proof refuses it in its own place.
pub fn with_links_taken<'a>(
    plan: &Plan<'a>,
    prefabs: &crate::compiler::registry::PrefabRegistry,
    world: &World,
) -> Result<Option<Plan<'a>>, Failure> {
    if plan.links.is_empty() {
        return Ok(None);
    }
    let takes = take_links(plan, world).unwrap_or_default();
    if takes.is_empty() {
        return Ok(None);
    }
    let relinked = plan.relinked(prefabs, takes).map_err(|e| e.failure)?;
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
    route_with_links(
        world,
        &positions_of(start, &cp.steps, &cp.transport_by_step),
        region_events,
        ancestor,
        &Carries {
            links: &plan.links,
            gathers: &plan.gathers,
            live: &cp.live_links_by_step,
            steps: &cp.steps,
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
            ) && let Some(path) = leg.find_path(a, b)
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
        let mut over: BTreeMap<[i32; 3], Option<&str>> = BTreeMap::new();
        for ((lo, hi), block) in &self.state.laid {
            for c in crate::compiler::assembled::region_cells(*lo, *hi) {
                over.insert(c, block.as_deref());
            }
        }
        over.iter()
            .filter(|(c, b)| load.get(*c).map(|s| s.as_str()) != **b)
            .count()
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
        w.find_path(start, goal)
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
    route_with_links(
        world,
        &positions_of(start, &cp.steps, &cp.transport_by_step),
        region_events,
        ancestor,
        &Carries {
            links: &plan.links,
            gathers: &plan.gathers,
            live: &cp.live_links_by_step,
            steps: &cp.steps,
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
pub fn branch_path_routes(
    world: &World,
    start: Option<[i32; 3]>,
    steps: &[Step],
    transports: &[Option<[i32; 3]>],
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Vec<LegRoute> {
    route_walked_legs(
        world,
        &positions_of(start, steps, transports),
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
            if !leg_world.is_standable(cell) {
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
