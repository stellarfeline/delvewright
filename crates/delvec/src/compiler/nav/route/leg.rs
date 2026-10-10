//! One leg's judgement and the link-aware routing of a whole path: the walk
//! proof that refuses a leg by its cause, and the links that carry a leg no
//! walk crosses ([`RouteBinding`]).

use crate::compiler::failure::Failure;
use crate::compiler::nav::route::region::{
    RegionState, gate_blame, held_blame_over, unforced_blame_over,
};
use crate::compiler::nav::route::{LegRoute, VisitedPos};
use crate::compiler::nav::world::World;
use crate::compiler::nav::world::body::SNAP_RADIUS;
use crate::compiler::nav::{
    DW_CLIMB_UNHELD, DW_CRITICAL_UNROUTABLE, DW_FLUID_FILL_ON_CRITICAL_PATH, DW_GATE_NEVER_OPENED,
    DW_LETHAL_ON_CRITICAL_PATH, DW_UNFORCED_FOOTING,
};
use crate::compiler::plan::{Plan, RegionEvents, Step};
use std::collections::{BTreeMap, BTreeSet};

/// Route every walked leg between consecutive visited positions over its
/// causally-sealed world ([`World::walked_leg_region_state`]), returning the proven cell routes. The
/// shared core of [`World::walked_legs`] and [`critical_path_routes`]; a leg whose
/// endpoints do not snap or that does not route is omitted (that is exactly the
/// [`route_visited`] failure, reported there as `DW0311`).
pub(in crate::compiler::nav) fn route_walked_legs(
    world: &World,
    positions: &[VisitedPos],
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Vec<(LegRoute, BTreeSet<[i32; 3]>)> {
    let mut out = Vec::new();
    for pair in positions.windows(2) {
        if pair[1].transport_before {
            continue; // an inter-area teleport hop: the player is moved, not walking
        }
        let st = world.walked_leg_region_state(
            region_events,
            ancestor,
            pair[0].src_step,
            pair[1].src_step,
        );
        let leg_world_owned;
        let leg_world: &World = if st.is_empty() {
            world
        } else {
            leg_world_owned = world.with_region_state(&st);
            &leg_world_owned
        };
        let sealed = st.solid.clone();
        let (Some(start), Some(goal)) = (
            leg_world.snap_endpoint(pair[0].pos, false),
            leg_world.snap_endpoint(pair[1].pos, pair[1].talk_to),
        ) else {
            continue;
        };
        if let Some(cells) = leg_world.find_path(start, goal) {
            let use_gates = cells
                .iter()
                .copied()
                .filter(|&c| leg_world.is_use_gate(c))
                .collect();
            let climbs = leg_world.climb_runs(&cells);
            out.push((
                LegRoute {
                    from: pair[0].pos,
                    to: pair[1].pos,
                    to_step: pair[1].src_step,
                    cells,
                    use_gates,
                    climbs,
                    // The leg carries the world it was PROVEN over, not just the
                    // route. Anything that re-judges these cells has to ask this
                    // value for the world to judge them in — see
                    // [`LegRoute::proven_world`].
                    region_state: st,
                },
                sealed,
            ));
        }
    }
    out
}

/// Route every walked leg between consecutive visited positions (the pure core of
/// [`check_critical_path`], split out so it is unit-testable without a full
/// [`Plan`]). A `transport_before` leg is a teleport ride and is skipped. Each leg
/// is routed over the world with any gate sealed by an earlier `close-gate`
/// ([`World::walked_leg_region_state`]) forced solid, so a forced path that must re-cross a sealed gate
/// fails [`DW_CRITICAL_UNROUTABLE`].
/// Render a blamed-volume list for a `DW0510` message: backticked ids joined by
/// `, `, or the honest `(none — the volume set is empty)` when the counterfactual
/// found a route that touches no declared volume, which would itself be a bug in
/// this proof rather than in the campaign.
fn names_of(ids: &[&str]) -> String {
    if ids.is_empty() {
        return "(none — the counterfactual route touches no declared volume; this is a \
                compiler defect, escalate it)"
            .to_string();
    }
    ids.iter()
        .map(|i| format!("`{i}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// `DW0510`'s **furniture shape** (spec-0065 §4.3): a walked route exists only
/// when the declared furniture is lifted, so the only way from `from` to `to`
/// climbs over it. One code with the lethal shape, because it is one rule — a
/// route may not depend on a cell a body may not be proven to stand in — and
/// the message names the kind.
pub(in crate::compiler::nav) fn furniture_route_failure(
    walker: &str,
    from: &str,
    to: &str,
    tables: &[&str],
) -> Failure {
    Failure {
        code: DW_LETHAL_ON_CRITICAL_PATH,
        message: format!(
            "{walker}: the only route from {from} to {to} runs OVER furniture {names} — the \
             piece that built it declares it a place a body stands beside and never on, and a \
             route that climbs a table is one nobody reads as a way. The geometry is walkable; \
             the declaration is what closes it. Move the mark so the walk has somewhere else to \
             go, open a way round the furniture, or move the furniture in the piece; do NOT \
             delete the declaration to silence the proof.",
            names = names_of(tables),
        ),
    }
}

/// Render a blamed-region list for a `DW0544` message. A runtime region write has
/// no author-given id — `fill-region` names a box, not itself — so the box IS the
/// name, and it identifies the effect uniquely. The empty case is the same honest
/// admission [`names_of`] makes: the counterfactual found a route that touches no
/// fluid box, which would be a defect in this proof rather than in the campaign.
fn boxes_of(regions: &[([i32; 3], [i32; 3])]) -> String {
    if regions.is_empty() {
        return "(none — the counterfactual route touches no fluid-filled box; this is a \
                compiler defect, escalate it)"
            .to_string();
    }
    regions
        .iter()
        .map(|(lo, hi)| {
            format!(
                "[{}, {}, {}]..[{}, {}, {}]",
                lo[0], lo[1], lo[2], hi[0], hi[1], hi[2]
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// **What the route proof may be carried by** (spec-0083): the plan's links and
/// gathers, the links live at each step of the path being judged, and that
/// path's steps — so a performed trigger can be recognised as a link.
pub(crate) struct Carries<'p> {
    pub(in crate::compiler::nav) links: &'p [crate::compiler::link::LinkPlan],
    pub(in crate::compiler::nav) gathers: &'p [crate::compiler::link::GatherPlan],
    pub(in crate::compiler::nav) live: &'p [Vec<usize>],
    pub(in crate::compiler::nav) steps: &'p [Step],
}

impl<'p> Carries<'p> {
    /// Nothing to be carried by: the walk proof alone.
    #[cfg(test)]
    fn none() -> Carries<'static> {
        Carries {
            links: &[],
            gathers: &[],
            live: &[],
            steps: &[],
        }
    }

    /// The default path's carry record.
    pub(in crate::compiler::nav) fn of_plan(plan: &'p Plan) -> Self {
        Carries {
            links: &plan.links,
            gathers: &plan.gathers,
            live: &plan.critical_path_live_links,
            steps: &plan.critical_path,
        }
    }

    /// The links live at path step `k`.
    fn live_at(&self, k: usize) -> &[usize] {
        self.live.get(k).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// **What the walk proof judged** (`DW0311`, spec-0083 §5): every leg of the
/// path partitioned by how the party crosses it, and the links and gathers the
/// campaign declares. Printed on every build, whichever way the proof goes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteBinding {
    /// Consecutive pairs of visited positions — visited positions minus one.
    pub legs: usize,
    /// Of those, legs the party walks.
    pub walked: usize,
    /// Of those, legs the compiler's own crossing carries.
    pub crossings: usize,
    /// Of those, legs a link carries.
    pub carried: usize,
    /// Of those, legs a loop's exercise move carries (spec-0086 §5.2).
    pub looped: usize,
    /// Links the campaign declares.
    pub links: usize,
    /// Of those, links live at the end of some leg.
    pub live: usize,
    /// Of those, links the path takes.
    pub taken: usize,
    /// Gathers the campaign declares.
    pub gathers: usize,
}

impl RouteBinding {
    /// The one line a build prints about this proof.
    pub fn line(&self) -> String {
        format!(
            "DW0311 binding: {} leg(s); {} walked, {} carried by a crossing, {} carried by a link, \
             {} carried by a loop; {} link(s) declared, {} live on some leg, {} taken; {} gather(s) \
             declared",
            self.legs,
            self.walked,
            self.crossings,
            self.carried,
            self.looped,
            self.links,
            self.live,
            self.taken,
            self.gathers
        )
    }
}

/// The cells of `l`'s volume a body can stand in and perform its trigger from
/// (spec-0083 §3.2), in cell order: standable on `w`, and — for a click — an
/// eye within a strike of the body's box ([`crate::compiler::strand::strikes`], the
/// rule `DW0924` reads), or — for `approach` — within the trigger's range of
/// its anchor.
pub(in crate::compiler::nav) fn stand_cells(
    w: &World,
    l: &crate::compiler::link::LinkPlan,
) -> Vec<[i32; 3]> {
    let (lo, hi) = l.from;
    let mut out = Vec::new();
    for x in lo[0]..=hi[0] {
        for y in lo[1]..=hi[1] {
            for z in lo[2]..=hi[2] {
                let c = [x, y, z];
                if !w.is_standable(c) {
                    continue;
                }
                let reaches = match l.range {
                    Some(r) => l.body.iter().any(|b| {
                        let dx = f64::from(c[0]) + 0.5 - f64::from(b[0]);
                        let dy = w.feet_y(c) - f64::from(b[1]);
                        let dz = f64::from(c[2]) + 0.5 - f64::from(b[2]);
                        (dx * dx + dy * dy + dz * dz).sqrt() <= f64::from(r)
                    }),
                    None => l
                        .body
                        .iter()
                        .any(|b| crate::compiler::strand::strikes(w, c, *b, 1.0, 1.0)),
                };
                if reaches {
                    out.push(c);
                }
            }
        }
    }
    out
}

/// **The cells outside `l`'s volume a body can perform its trigger from**
/// (spec-0092 §10): standable, in the walk region `reachable`, outside `from`,
/// and reaching the trigger by the same rule [`stand_cells`] reads — an eye
/// within a strike of the body's box for a click, and in sight of it
/// ([`sees_body`]), the trigger's range for an `approach` — in cell order. Searched within five cells of each body cell,
/// which holds every cell a strike or an approach of five reaches from.
pub fn press_cells_outside(
    w: &World,
    l: &crate::compiler::link::LinkPlan,
    reachable: &BTreeSet<[i32; 3]>,
) -> Vec<[i32; 3]> {
    let reach = 5 + l.range.map_or(0, |r| r as i32);
    let mut out: BTreeSet<[i32; 3]> = BTreeSet::new();
    for b in &l.body {
        for x in b[0] - reach..=b[0] + reach {
            for y in b[1] - reach..=b[1] + reach {
                for z in b[2] - reach..=b[2] + reach {
                    let c = [x, y, z];
                    if l.contains(c) || !reachable.contains(&c) || !w.is_standable(c) {
                        continue;
                    }
                    let presses = match l.range {
                        Some(r) => {
                            let dx = f64::from(c[0]) + 0.5 - f64::from(b[0]);
                            let dy = w.feet_y(c) - f64::from(b[1]);
                            let dz = f64::from(c[2]) + 0.5 - f64::from(b[2]);
                            (dx * dx + dy * dy + dz * dz).sqrt() <= f64::from(r)
                        }
                        None => {
                            crate::compiler::strand::strikes(w, c, *b, 1.0, 1.0)
                                && sees_body(w, c, *b)
                        }
                    };
                    if presses {
                        out.insert(c);
                    }
                }
            }
        }
    }
    out.into_iter().collect()
}

/// Whether an eye standing in `p` sees some point of a click body standing on
/// `m` (its `1 x 2` box) past every solid cell — the box's centre or one of its
/// eight corners pulled a tenth of a block inward, each sought along the line
/// from the eye in tenth-of-a-block steps. A press through a wall is no press:
/// the client's pick stops at the first block it meets.
fn sees_body(w: &World, p: [i32; 3], m: [i32; 3]) -> bool {
    use delvewright_dsl::metrics::PLAYER_EYE_HEIGHT;
    let eye = [
        f64::from(p[0]) + 0.5,
        w.feet_y(p) + PLAYER_EYE_HEIGHT,
        f64::from(p[2]) + 0.5,
    ];
    let (x0, y0, z0) = (f64::from(m[0]), f64::from(m[1]), f64::from(m[2]));
    let mut targets = vec![[x0 + 0.5, y0 + 1.0, z0 + 0.5]];
    for dx in [0.1, 0.9] {
        for dy in [0.1, 1.9] {
            for dz in [0.1, 0.9] {
                targets.push([x0 + dx, y0 + dy, z0 + dz]);
            }
        }
    }
    targets.iter().any(|t| {
        let d = [t[0] - eye[0], t[1] - eye[1], t[2] - eye[2]];
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let steps = (len / 0.1).ceil().max(1.0) as i32;
        (1..steps).all(|i| {
            let f = f64::from(i) / f64::from(steps);
            let c = [
                (eye[0] + d[0] * f).floor() as i32,
                (eye[1] + d[1] * f).floor() as i32,
                (eye[2] + d[2] * f).floor() as i32,
            ];
            c == p || c == m || c == [m[0], m[1] + 1, m[2]] || !w.solid.contains(&c)
        })
    })
}

/// Whether `l`'s `to` is a cell a body stands on in the world as the link's own
/// root leaves it at the teleport's tick (spec-0083 §3.6): `st` (the leg's
/// region state), with the root's writes at an earlier tick applied — forced,
/// since they fire with it — and its fills at the teleport's tick or later not
/// yet laid.
pub(in crate::compiler::nav) fn to_standable(
    w: &World,
    st: &RegionState,
    l: &crate::compiler::link::LinkPlan,
) -> bool {
    let mut st = st.clone();
    for wr in &l.writes {
        let cells: Vec<[i32; 3]> =
            crate::compiler::assembled::region_cells(wr.region.0, wr.region.1).collect();
        if wr.tick >= l.tick {
            if wr.write.fills() {
                for c in &cells {
                    st.solid.remove(c);
                    st.unforced.remove(c);
                }
            }
            continue;
        }
        for c in cells {
            st.solid.remove(&c);
            st.cleared.remove(&c);
            st.flooded.remove(&c);
            st.unforced.remove(&c);
            match wr.write {
                crate::compiler::plan::RegionWrite::Flood => {
                    st.flooded.insert(c);
                }
                crate::compiler::plan::RegionWrite::Fill => {
                    st.solid.insert(c);
                }
                crate::compiler::plan::RegionWrite::Clear
                | crate::compiler::plan::RegionWrite::Unseal
                | crate::compiler::plan::RegionWrite::Pass => {
                    st.cleared.insert(c);
                }
            }
        }
    }
    let owned;
    let at: &World = if st.is_empty() {
        w
    } else {
        owned = w.with_region_state(&st);
        &owned
    };
    at.is_standable(l.to)
}

/// Why one link did not carry a leg, for `DW0311`'s message.
enum LinkMiss {
    /// Its volume holds a stand cell, and the leg's start cannot walk to any.
    Unreachable,
    /// The party lands at `to` and cannot walk on to the leg's end.
    NoOnward,
}

/// The nearest stand cell of `cands` to `from` by proven route length, ties
/// broken by cell order (ADR-0006) — `None` when none is reachable.
fn nearest_stand(w: &World, from: &VisitedPos, cands: &[[i32; 3]]) -> Option<[i32; 3]> {
    let start = w.snap_endpoint(from.pos, false)?;
    cands
        .iter()
        .filter_map(|c| w.find_path(start, *c).map(|p| (p.len(), *c)))
        .min()
        .map(|(_, c)| c)
}

/// Retry the leg `from → end` through the links live at `end` (spec-0083 §3.4):
/// walk to a stand cell, be carried, and walk on — the last segment itself
/// retried through the links not yet used on this leg. The first decomposition
/// that routes is the leg's route, pushed onto `used` in the order taken.
#[allow(clippy::too_many_arguments)]
fn decompose(
    world: &World,
    from: &VisitedPos,
    end: &VisitedPos,
    origin_step: usize,
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
    carries: &Carries<'_>,
    used: &mut Vec<(usize, [i32; 3])>,
    misses: &mut BTreeMap<usize, LinkMiss>,
    faults: &mut Vec<(usize, String)>,
) -> bool {
    let st = world.walked_leg_region_state(region_events, ancestor, origin_step, end.src_step);
    let owned;
    let leg: &World = if st.is_empty() {
        world
    } else {
        owned = world.with_region_state(&st);
        &owned
    };
    for &li in carries.live_at(end.src_step) {
        if used.iter().any(|(u, _)| *u == li) {
            continue;
        }
        let Some(l) = carries.links.get(li) else {
            continue;
        };
        let cands = stand_cells(leg, l);
        if cands.is_empty() {
            faults.push((
                li,
                format!(
                    "no standable cell inside its volume {} performs the trigger — the body it \
                     presses stands outside its own volume, so whoever presses it is not carried. \
                     Fault: no stand cell. Remedy: move the body or widen the volume so a cell \
                     inside it reaches the body",
                    l.box_words()
                ),
            ));
            continue;
        }
        if !to_standable(world, &st, l) {
            faults.push((
                li,
                format!(
                    "its `to` {:?} is not a cell a body stands on at the teleport's tick {} — a \
                     route position is a cell a body stands on, and a link onto air is a drop. \
                     Fault: `to` not standable. Remedy: move `to` off the volume and onto footing, \
                     or lay the floor at an earlier tick of the same root",
                    l.to, l.tick
                ),
            ));
            continue;
        }
        let Some(stand) = nearest_stand(leg, from, &cands) else {
            misses.entry(li).or_insert(LinkMiss::Unreachable);
            continue;
        };
        let stand_vp = VisitedPos {
            pos: stand,
            transport_before: false,
            talk_to: false,
            src_step: end.src_step,
            by_link: false,
            by_loop: false,
        };
        if judge_leg(world, &[*from, stand_vp], region_events, ancestor).is_err() {
            misses.entry(li).or_insert(LinkMiss::Unreachable);
            continue;
        }
        let to_vp = VisitedPos {
            pos: l.to,
            transport_before: false,
            talk_to: false,
            src_step: origin_step,
            by_link: true,
            by_loop: false,
        };
        used.push((li, stand));
        if judge_leg(world, &[to_vp, *end], region_events, ancestor).is_ok() {
            return true;
        }
        if decompose(
            world,
            &to_vp,
            end,
            origin_step,
            region_events,
            ancestor,
            carries,
            used,
            misses,
            faults,
        ) {
            return true;
        }
        used.pop();
        misses.insert(li, LinkMiss::NoOnward);
    }
    false
}

/// The link a performed `trigger` step stands for, and the cell the party
/// stands on to perform it from `from` — `None` when the step's trigger is no
/// link, or no stand cell of its volume is reachable from `from` (the press
/// then happens outside the box and carries nobody).
fn performed_link(
    world: &World,
    from: &VisitedPos,
    step: usize,
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
    carries: &Carries<'_>,
) -> Result<Option<(usize, [i32; 3])>, Failure> {
    let Some(Step::Trigger {
        trigger_id,
        stand: None,
        ..
    }) = carries.steps.get(step)
    else {
        return Ok(None);
    };
    let st = world.walked_leg_region_state(region_events, ancestor, from.src_step, step);
    let owned;
    let leg: &World = if st.is_empty() {
        world
    } else {
        owned = world.with_region_state(&st);
        &owned
    };
    for (li, l) in carries.links.iter().enumerate() {
        if &l.trigger_id != trigger_id {
            continue;
        }
        let cands = stand_cells(leg, l);
        let Some(stand) = nearest_stand(leg, from, &cands) else {
            continue;
        };
        if !to_standable(world, &st, l) {
            return Err(Failure {
                code: crate::compiler::plan::DW_TELEPORT_LINK,
                message: format!(
                    "link `{}` (the `teleport` at `{}`) is performed by the path from {stand:?}, \
                     inside its volume {}, and puts the party on {:?}, which is not a cell a body \
                     stands on at the teleport's tick {} — a route position is a cell a body \
                     stands on, and a link onto air is a drop. Fault: `to` not standable. \
                     Remedy: move `to` off the volume and onto footing, or lay the floor at an \
                     earlier tick of the same root.",
                    l.trigger_id,
                    l.path,
                    l.box_words(),
                    l.to,
                    l.tick
                ),
            });
        }
        return Ok(Some((li, stand)));
    }
    Ok(None)
}

/// The message for a leg no walk and no link carries (spec-0083 §3.1, §3.4):
/// under `DW0311` a gather over the leg's start is named with the remedy that
/// makes it a link; under any code of the leg family, every link considered on
/// the leg is named with why it did not carry.
fn widen_unroutable(
    e: Failure,
    from: &VisitedPos,
    end: &VisitedPos,
    carries: &Carries<'_>,
    misses: &BTreeMap<usize, LinkMiss>,
) -> Failure {
    let considered: Vec<String> = carries
        .links
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let why = if !carries.live_at(end.src_step).contains(&i) {
                "shut at this step — its trigger's gate or the teleport's `when` does not hold \
                 under what the path holds here"
            } else {
                match misses.get(&i) {
                    Some(LinkMiss::Unreachable) => {
                        "its volume holds a stand cell and the leg's start cannot walk to one"
                    }
                    Some(LinkMiss::NoOnward) => "the party lands at its `to` and cannot walk on",
                    None => "not tried",
                }
            };
            format!("`{}` (`{}`): {why}", l.trigger_id, l.path)
        })
        .collect();
    if e.code == DW_CRITICAL_UNROUTABLE
        && let Some(g) = carries.gathers.iter().find(|g| g.contains(from.pos))
    {
        return Failure {
            code: DW_CRITICAL_UNROUTABLE,
            message: format!(
                "critical path: the party cannot walk from {:?} to {:?}, and the way the campaign \
                 means is the `teleport` at `{}` — its volume holds the leg's start, and it \
                 carries whoever is inside to {:?}. It fires from {}, a root the party cannot \
                 fire again: the first body through it travels and the rest of the party is left \
                 behind, so the proof does not lean on it. Host the same `teleport` in a \
                 `triggers[]` entry of kind `use`, `strike` or `approach` declared \
                 `\"once\": false`, whose volume holds a cell a body presses it from — a link a \
                 straggler can take again.{}",
                from.pos,
                end.pos,
                g.path,
                g.to,
                g.root,
                if considered.is_empty() {
                    String::new()
                } else {
                    format!(" Links considered on this leg: {}.", considered.join("; "))
                }
            ),
        };
    }
    if considered.is_empty() {
        return e;
    }
    Failure {
        code: e.code,
        message: format!(
            "{} Links considered on this leg, none of which carries it: {}.",
            e.message,
            considered.join("; ")
        ),
    }
}

/// **The walk proof, with links** (spec-0083 §3.4): every leg of `positions`
/// is walked; a leg whose walk fails is retried through the links live at its
/// end; a performed `trigger` step whose trigger is a link the party stands in
/// carries it. Returns the binding (always), and the links taken beside the
/// verdict — keyed by the step indices of the path judged — so the build can
/// splice them into the one path every consumer reads.
pub(in crate::compiler::nav) fn route_with_links(
    world: &World,
    positions: &[VisitedPos],
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
    carries: &Carries<'_>,
) -> (
    RouteBinding,
    Result<crate::compiler::plan::LinkTakes, Failure>,
) {
    // The leg count is the population, stated before any leg is judged — the
    // visited positions minus one, plus the legs a taken link adds — so a
    // refusal prints the same denominator a pass does.
    let mut b = RouteBinding {
        legs: positions.len().saturating_sub(1),
        links: carries.links.len(),
        gathers: carries.gathers.len(),
        ..RouteBinding::default()
    };
    let mut takes = crate::compiler::plan::LinkTakes::default();
    let mut live: BTreeSet<usize> = BTreeSet::new();
    let mut taken: BTreeSet<String> = BTreeSet::new();
    let Some(first) = positions.first() else {
        return (b, Ok(takes));
    };
    let mut cur = *first;
    let mut result = Ok(());
    for next in &positions[1..] {
        live.extend(carries.live_at(next.src_step).iter().copied());
        if next.transport_before {
            if next.by_link {
                b.carried += 1;
                if let Some(Step::Trigger { trigger_id, .. }) = carries.steps.get(next.src_step) {
                    taken.insert(format!("{trigger_id}{:?}", next.pos));
                }
            } else if next.by_loop {
                b.looped += 1;
            } else {
                b.crossings += 1;
            }
            cur = *next;
            continue;
        }
        // A trigger the path performs that is a link the party can stand in:
        // the leg into it ends at the stand cell, and the party goes on from
        // `to` (spec-0083 §3.4, last sentence).
        match performed_link(world, &cur, next.src_step, region_events, ancestor, carries) {
            Err(e) => {
                result = Err(e);
                break;
            }
            Ok(Some((li, stand))) => {
                let l = &carries.links[li];
                let stand_vp = VisitedPos {
                    pos: stand,
                    ..*next
                };
                if let Err(e) = judge_leg(world, &[cur, stand_vp], region_events, ancestor) {
                    result = Err(e);
                    break;
                }
                // `cur → trigger` became `cur → stand`, `stand → to`.
                b.legs += 1;
                b.walked += 1;
                b.carried += 1;
                taken.insert(format!("{}{:?}", l.trigger_id, l.to));
                takes.performed.insert(next.src_step, (li, stand));
                // The next leg starts here; whether a leg is a ride is read off
                // its END, so this start marks nothing.
                cur = VisitedPos {
                    pos: l.to,
                    transport_before: false,
                    talk_to: false,
                    src_step: next.src_step,
                    by_link: true,
                    by_loop: false,
                };
                continue;
            }
            Ok(None) => {}
        }
        let walked = judge_leg(world, &[cur, *next], region_events, ancestor);
        let Err(e) = walked else {
            b.walked += 1;
            cur = *next;
            continue;
        };
        let mut used = Vec::new();
        let mut misses = BTreeMap::new();
        let mut faults = Vec::new();
        if decompose(
            world,
            &cur,
            next,
            cur.src_step,
            region_events,
            ancestor,
            carries,
            &mut used,
            &mut misses,
            &mut faults,
        ) {
            b.legs += 2 * used.len();
            b.walked += 1 + used.len();
            b.carried += used.len();
            for (li, _) in &used {
                let l = &carries.links[*li];
                taken.insert(format!("{}{:?}", l.trigger_id, l.to));
            }
            takes.spliced.insert(next.src_step, used);
            cur = *next;
            continue;
        }
        // The climb counterfactual, through the links (spec-0099): a leg whose
        // walk the unheld-climb credit alone cannot mend (`judge_leg` asked that)
        // may still be one the credit mends once the party is carried part of the
        // way — out of a sealed cabin, then up a ladder that is not there.
        if world.has_unheld_climbs() && e.code != DW_CLIMB_UNHELD {
            let credited = world.with_unheld_climbs();
            let (mut used2, mut misses2, mut faults2) = (Vec::new(), BTreeMap::new(), Vec::new());
            if decompose(
                &credited,
                &cur,
                next,
                cur.src_step,
                region_events,
                ancestor,
                carries,
                &mut used2,
                &mut misses2,
                &mut faults2,
            ) {
                let mut cells: Vec<[i32; 3]> = Vec::new();
                let mut at = cur.pos;
                let mut talk = false;
                let ends: Vec<([i32; 3], [i32; 3])> = used2
                    .iter()
                    .map(|(li, stand)| (*stand, carries.links[*li].to))
                    .collect();
                for (stand, to) in ends
                    .iter()
                    .copied()
                    .chain(std::iter::once((next.pos, next.pos)))
                {
                    if stand == next.pos {
                        talk = next.talk_to;
                    }
                    if let (Some(a), Some(b)) = (
                        credited.snap_endpoint(at, false),
                        credited.snap_endpoint(stand, talk),
                    ) && let Some(path) = credited.find_path(a, b)
                    {
                        cells.extend(path);
                    }
                    at = to;
                }
                result = Err(Failure {
                    code: DW_CLIMB_UNHELD,
                    message: format!(
                        "critical path: the party cannot get from {:?} to {:?}, walking or \
                         carried, and the way it would take climbs a climbable the world does \
                         not keep — {}. The game removes a ladder or a vine whose hold fails at \
                         the first shape update that reaches it, so the climb the piece shows is \
                         not there to take. Give it its hold — a full face on the block it hangs \
                         on — or route the forced path another way; do NOT move the objective \
                         to dodge the climb.",
                        cur.pos,
                        next.pos,
                        world.unheld_climbs_words(&cells),
                    ),
                });
                break;
            }
        }
        if let Some((li, why)) = faults.first() {
            let l = &carries.links[*li];
            result = Err(Failure {
                code: crate::compiler::plan::DW_TELEPORT_LINK,
                message: format!(
                    "critical path: the party cannot walk from {:?} to {:?}, and the only carry \
                     on that leg is link `{}` (the `teleport` at `{}`, volume {}, `to` {:?}), \
                     whose geometry does not hold: {why}.",
                    cur.pos,
                    next.pos,
                    l.trigger_id,
                    l.path,
                    l.box_words(),
                    l.to
                ),
            });
            break;
        }
        result = Err(widen_unroutable(e, &cur, next, carries, &misses));
        break;
    }
    b.live = live.len().min(b.links);
    b.taken = taken.len();
    (b, result.map(|()| takes))
}

/// Route every walked leg between consecutive visited positions with no link to
/// lean on — [`route_with_links`] over an empty carry record. The pure core the
/// unit tests drive over synthetic worlds.
#[cfg(test)]
pub(in crate::compiler::nav) fn route_visited(
    world: &World,
    positions: &[VisitedPos],
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<(), Failure> {
    route_with_links(world, positions, region_events, ancestor, &Carries::none())
        .1
        .map(|_| ())
}

/// Judge ONE walked leg, `pair[0] → pair[1]`, over its causally-sealed world —
/// every refusal `DW0311` and its family give for a leg that does not route. A
/// `transport_before` leg is a ride and is not judged.
fn judge_leg(
    world: &World,
    pair: &[VisitedPos],
    region_events: &RegionEvents,
    ancestor: &dyn Fn(usize, usize) -> bool,
) -> Result<(), Failure> {
    {
        let from = pair[0].pos;
        let to = pair[1].pos;
        if pair[1].transport_before {
            return Ok(()); // a carry: the player is moved, not walking
        }
        let st = world.walked_leg_region_state(
            region_events,
            ancestor,
            pair[0].src_step,
            pair[1].src_step,
        );
        let leg_world_owned;
        let leg_world: &World = if st.is_empty() {
            world
        } else {
            leg_world_owned = world.with_region_state(&st);
            &leg_world_owned
        };
        // The unforced counterfactual is built HERE, while `st` is still whole: the
        // world as it would be if every fill laid by a skippable beat were credited
        // as ordinary floor, which is precisely the model this compiler ran before
        // forcedness reached the geometry. Built only for a leg that has such a fill
        // at all — every other campaign routes over the identical single world and
        // pays nothing. Its blame ledger is taken by value for the same reason.
        let unforced_regions = st.unforced_regions.clone();
        let has_unforced = !st.unforced.is_empty();
        // The loop counterfactual (spec-0086 §5.1): this leg with every holding
        // slab released. Built only for a leg that has one, so every campaign
        // without a loop routes over the identical single world.
        let held_regions = st.held_regions.clone();
        let released_owned;
        let released: Option<&World> = if st.held.is_empty() {
            None
        } else {
            released_owned = world.with_region_state(&st.released());
            Some(&released_owned)
        };
        let credited_owned;
        let credited: Option<&World> = if st.unforced.is_empty() {
            None
        } else {
            credited_owned = world.with_region_state(&st.as_if_forced());
            Some(&credited_owned)
        };
        let sealed = st.solid;
        let flooded = st.flooded;
        // The lethal-free view of this same leg, built once and only when the
        // campaign declares a volume at all. Every failure below asks it first:
        // a leg that routes here and nowhere else failed *because of* lethality,
        // and saying which volume is a different fix from every other answer
        // this function gives.
        let open_owned;
        let open: Option<&World> = if leg_world.has_lethal() || leg_world.has_furniture() {
            open_owned = leg_world.without_exclusions();
            Some(&open_owned)
        } else {
            None
        };
        // The same leg as this compiler saw it before the world-load gate seals
        // were measured: every gate assumed open. Built once, and only for a
        // campaign whose world actually authors a gate shut — everyone else routes
        // over the identical single world and pays nothing.
        let ungated_owned;
        let ungated: Option<&World> = if !world.has_world_load_seals() {
            None
        } else {
            let st2 = world.leg_region_state_without_world_load(
                region_events,
                ancestor,
                pair[0].src_step,
                pair[1].src_step,
            );
            ungated_owned = if st2.is_empty() {
                world.with_region_state(&RegionState::default())
            } else {
                world.with_region_state(&st2)
            };
            Some(&ungated_owned)
        };
        let lethal_snap_err = |at: [i32; 3], talk_to: bool| -> Option<Failure> {
            let open = open?;
            let cell = open.snap_endpoint(at, talk_to)?;
            let volumes = leg_world.lethal_volumes_over(&[cell]);
            let tables = leg_world.furniture_over(&[cell]);
            if volumes.is_empty() && !tables.is_empty() {
                return Some(Failure {
                    code: DW_LETHAL_ON_CRITICAL_PATH,
                    message: format!(
                        "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                         anchor {at:?} is ON furniture {names} — the piece that built it \
                         declares it a place a body stands beside and never on, so the party \
                         cannot be proven to stand where this objective is. Move the objective \
                         to the floor beside it, or move the furniture in the piece; do NOT \
                         delete the declaration to silence the proof.",
                        names = names_of(&tables),
                    ),
                });
            }
            let names = names_of(&volumes);
            let staged = world.staged_words(&volumes, region_events, pair[1].src_step, ancestor);
            Some(Failure {
                code: DW_LETHAL_ON_CRITICAL_PATH,
                message: format!(
                    "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                     anchor {at:?} lies INSIDE lethal volume(s) {names} — a player who reaches \
                     this objective is killed by standing where the objective is.{staged} Move \
                     the volume off the anchor, shrink its `extent`, or move the objective; do \
                     NOT delete the volume to silence the proof."
                ),
            })
        };
        // The gate counterfactual, in the same shape and for the same reason as the
        // lethal one: an anchor whose only footing is inside a gate region the world
        // authors shut needs to be told WHICH door, not sent to look for a wedged
        // prefab that is fine.
        let gate_snap_err = |at: [i32; 3], talk_to: bool| -> Option<Failure> {
            let ungated = ungated?;
            let cell = ungated.snap_endpoint(at, talk_to)?;
            let blamed = gate_blame(
                &world.gate_seals_over(&[cell]),
                region_events,
                ancestor,
                pair[1].src_step,
            );
            Some(Failure {
                code: DW_GATE_NEVER_OPENED,
                message: format!(
                    "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                     anchor {at:?} is inside a gate region the placed world authors SHUT at \
                     world-load — {blamed}. The party can never stand where this objective is. \
                     Fire `open-gate` on that anchor from an objective the party is forced to \
                     complete before this one, or move the objective off the gate; do NOT delete \
                     the gate, and do NOT remove the anchor's fill block from the prefab to \
                     silence the proof."
                ),
            })
        };

        // The same move for the other premise that closes a leg while the geometry
        // reads open: a runtime fill of water or lava. Built once, and only for a
        // campaign that fills a region with a fluid at all — every other campaign
        // pays nothing and routes byte-identically.
        let dry_owned;
        let dry: Option<&World> = if leg_world.has_runtime_flood() {
            dry_owned = leg_world.without_runtime_flood();
            Some(&dry_owned)
        } else {
            None
        };
        let fluid_snap_err = |at: [i32; 3], talk_to: bool| -> Option<Failure> {
            let dry = dry?;
            let cell = dry.snap_endpoint(at, talk_to)?;
            let boxes = boxes_of(&leg_world.flood_regions_over(&[cell]));
            Some(Failure {
                code: DW_FLUID_FILL_ON_CRITICAL_PATH,
                message: format!(
                    "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                     anchor {at:?} is a cell a runtime region write fills with FLUID {boxes} — \
                     a body does not stand on water or lava, so the party arrives at this \
                     objective and sinks. Fill the box with a block that is floor, put the \
                     floor one cell below the fluid, or move the objective; do NOT silence \
                     this by filling with a solid you do not want in the world."
                ),
            })
        };
        // And the same blame move for the premise this family was missing: a solid
        // laid by a beat nobody has to play.
        let unforced_snap_err = |at: [i32; 3], talk_to: bool| -> Option<Failure> {
            let credited = credited?;
            let cell = credited.snap_endpoint(at, talk_to)?;
            let boxes = unforced_blame_over(&unforced_regions, &[cell]).join("; ");
            Some(Failure {
                code: DW_UNFORCED_FOOTING,
                message: format!(
                    "critical path: the only footing within {SNAP_RADIUS} blocks of visited \
                     anchor {at:?} is laid at runtime by a beat the party is NOT forced to play \
                     — {boxes}. A party that never plays that beat arrives at this objective \
                     and finds no ground to stand on. Fire the fill from an objective the party \
                     is FORCED to complete before this one, put the floor in the prefab, or move \
                     the objective; do NOT keep the beat optional and hope."
                ),
            })
        };
        let start = match leg_world.snap_endpoint(from, false) {
            Some(c) => c,
            None => {
                if let Some(e) = lethal_snap_err(from, false) {
                    return Err(e);
                }
                if let Some(e) = fluid_snap_err(from, false) {
                    return Err(e);
                }
                if let Some(e) = unforced_snap_err(from, false) {
                    return Err(e);
                }
                if let Some(e) = gate_snap_err(from, false) {
                    return Err(e);
                }
                return Err(Failure {
                    code: DW_CRITICAL_UNROUTABLE,
                    message: format!(
                        "critical path: no standable floor within {SNAP_RADIUS} blocks of visited \
                         anchor {from:?} — a player-visited anchor sits walled in or over void. \
                         Fix the prefab so this anchor sits on/next to reachable floor; if the \
                         prefab looks correct, this is an assembly/toolchain defect — escalate \
                         rather than move the anchor into a wall"
                    ),
                });
            }
        };
        let goal = match leg_world.snap_endpoint(to, pair[1].talk_to) {
            Some(c) => c,
            None => {
                if let Some(e) = lethal_snap_err(to, pair[1].talk_to) {
                    return Err(e);
                }
                if let Some(e) = fluid_snap_err(to, pair[1].talk_to) {
                    return Err(e);
                }
                if let Some(e) = unforced_snap_err(to, pair[1].talk_to) {
                    return Err(e);
                }
                if let Some(e) = gate_snap_err(to, pair[1].talk_to) {
                    return Err(e);
                }
                return Err(Failure {
                    code: DW_CRITICAL_UNROUTABLE,
                    message: format!(
                        "critical path: no standable floor within {SNAP_RADIUS} blocks of visited \
                         anchor {to:?} — a player-visited anchor sits walled in or over void (a \
                         talk-to NPC needs a dry standable cell beside it, within interaction \
                         range and clear of water). Fix the prefab so this anchor sits on/next to \
                         reachable floor; if the prefab looks correct, this is an \
                         assembly/toolchain defect — escalate rather than move it into a wall"
                    ),
                });
            }
        };
        if leg_world.find_path(start, goal).is_none() {
            // A holding loop first (spec-0086 §5.1): when the leg routes with the
            // slab released and not with it held, the corridor is endless for
            // this party here, and the remedy is a release, never a walk.
            if let Some(free) = released
                && let (Some(s2), Some(g2)) = (
                    free.snap_endpoint(from, false),
                    free.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = free.find_path(s2, g2)
            {
                let held = held_blame_over(&held_regions, &cells).join("; ");
                return Err(Failure {
                    code: DW_CRITICAL_UNROUTABLE,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) crosses {held}. A body that enters a holding \
                         slab is returned to the approach on every crossing, so the party never \
                         reaches the far side while the loop holds, and nothing the forced path \
                         performs before this leg releases it. Release the loop before this leg \
                         — set the flag its gate forbids, or raise the count it reads — from an \
                         objective the party is forced to complete first, or route the forced \
                         path so it does not cross the slab while the loop holds."
                    ),
                });
            }
            // Lethality first: it is the strictly more specific answer, and the
            // generic one below would send the author to fix open geometry.
            if let Some(open) = open
                && let (Some(s2), Some(g2)) = (
                    open.snap_endpoint(from, false),
                    open.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = open.find_path(s2, g2)
            {
                let volumes = leg_world.lethal_volumes_over(&cells);
                let tables = leg_world.furniture_over(&cells);
                if volumes.is_empty() && !tables.is_empty() {
                    return Err(furniture_route_failure(
                        "critical path",
                        &format!("{from:?} (floor {start:?})"),
                        &format!("{to:?} (floor {goal:?})"),
                        &tables,
                    ));
                }
                let names = names_of(&volumes);
                let staged =
                    world.staged_words(&volumes, region_events, pair[1].src_step, ancestor);
                return Err(Failure {
                    code: DW_LETHAL_ON_CRITICAL_PATH,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) runs THROUGH lethal volume(s) {names} — the \
                         party cannot reach this objective without dying on the way. The \
                         geometry is walkable; the volume is what closes it.{staged} Move or \
                         shrink the volume, give the party a route around it, or — for a volume \
                         live from a story stage — move the beat that arms it after this leg; \
                         do NOT delete the volume to silence the proof."
                    ),
                });
            }
            // Then the fluid fill, for the same reason and with the same shape: the
            // route the author believes in exists, and what closed it is a box they
            // filled with water or lava. Asked after lethality only because a
            // campaign that has both wants the answer that kills the party told
            // first; the two are otherwise independent.
            if let Some(dry) = dry
                && let (Some(s2), Some(g2)) = (
                    dry.snap_endpoint(from, false),
                    dry.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = dry.find_path(s2, g2)
            {
                let boxes = boxes_of(&leg_world.flood_regions_over(&cells));
                return Err(Failure {
                    code: DW_FLUID_FILL_ON_CRITICAL_PATH,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) needs footing inside FLUID-filled region(s) \
                         {boxes} — a runtime write fills that box with water or lava, and a \
                         body walks on neither. The geometry would carry the party if the box \
                         held a block; the fluid is what closes it. Fill with a block that is \
                         floor, drop the walkable surface to the cell below the fluid, or route \
                         the forced path around the box; do NOT swap in a solid you do not want \
                         in the world just to get green."
                    ),
                });
            }
            // Then the unforced fill, for the same reason and in the same shape: the
            // route the author believes in exists, and what closed it is that its
            // floor is laid by a beat the party can walk past. Asked before the gate
            // counterfactual because it is the more specific answer — a gate the
            // campaign never opens is about a door, this is about who has to press
            // it — and because an unforced fill over a gate region would otherwise
            // be reported as a missing `open-gate` the author has already written.
            if let Some(credited) = credited
                && let (Some(s2), Some(g2)) = (
                    credited.snap_endpoint(from, false),
                    credited.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = credited.find_path(s2, g2)
            {
                let boxes = unforced_blame_over(&unforced_regions, &cells).join("; ");
                return Err(Failure {
                    code: DW_UNFORCED_FOOTING,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) needs footing laid at runtime by a beat the \
                         party is NOT forced to play — {boxes}. The geometry would carry the \
                         party if that beat always fired; it does not, so a party that skips it \
                         is stranded here. The fill still SEALS for the proof — an unforced \
                         write may make a region impassable — it just may not be stood on. \
                         Move the fill onto an objective the party is FORCED to complete before \
                         this leg, build the floor into the prefab, or route the forced path \
                         around the box; do NOT leave the path depending on a beat that can be \
                         skipped."
                    ),
                });
            }
            // Then the gate counterfactual: the identical world as it would be if
            // every gate the placed prefabs author shut had been born open — which
            // is precisely the model this compiler used to ship. A leg that routes
            // there and nowhere else failed *because of a door*, and the repair is
            // a missing `open-gate` on a name, not a hunt through the geometry.
            if let Some(ungated) = ungated
                && let (Some(s2), Some(g2)) = (
                    ungated.snap_endpoint(from, false),
                    ungated.snap_endpoint(to, pair[1].talk_to),
                )
                && let Some(cells) = ungated.find_path(s2, g2)
            {
                let blamed = gate_blame(
                    &world.gate_seals_over(&cells),
                    region_events,
                    ancestor,
                    pair[1].src_step,
                );
                return Err(Failure {
                    code: DW_GATE_NEVER_OPENED,
                    message: format!(
                        "critical path: the only route from {from:?} (floor {start:?}) to \
                         {to:?} (floor {goal:?}) runs THROUGH a gate the placed world authors \
                         SHUT at world-load — {blamed}. The geometry is walkable and the prefab \
                         is right; the door is what closes it, and the party has no way to open \
                         it before they must be on the far side. Fire `open-gate` on that anchor \
                         from an objective the party is FORCED to complete before this leg, or \
                         route the forced path so it does not cross the gate. Do NOT delete the \
                         gate, and do NOT strip the anchor's fill block out of the prefab to \
                         silence the proof."
                    ),
                });
            }
            // Then the climb counterfactual (spec-0099): the same leg with every
            // climbable the world does NOT keep credited as if it hung there. A leg
            // that routes there and nowhere else is closed by a ladder or a vine
            // with nothing to hang on — the author can see it in the piece, and
            // the game removes it at the first shape update. The repair is its
            // hold, never the route.
            if leg_world.has_unheld_climbs() {
                let credited = leg_world.with_unheld_climbs();
                if let (Some(s2), Some(g2)) = (
                    credited.snap_endpoint(from, false),
                    credited.snap_endpoint(to, pair[1].talk_to),
                ) && let Some(cells) = credited.find_path(s2, g2)
                {
                    let named = leg_world.unheld_climbs_words(&cells);
                    return Err(Failure {
                        code: DW_CLIMB_UNHELD,
                        message: format!(
                            "critical path: the only route from {from:?} (floor {start:?}) to \
                             {to:?} (floor {goal:?}) climbs a climbable the world does not keep \
                             — {named}. The game removes a ladder or a vine whose hold fails at \
                             the first shape update that reaches it, so the climb the piece shows \
                             is not there to take. Give it its hold — a full face on the block \
                             it hangs on — or route the forced path another way; do NOT move the \
                             objective to dodge the climb."
                        ),
                    });
                }
            }
            // A leg the fluid did not *uniquely* close is still a leg a fluid may
            // have walled: `DW0544` fires only when the box supplied FOOTING, and a
            // flood laid across a corridor blocks it whether or not it is wet. That
            // is an unroutable leg the campaign built on purpose, so it may not be
            // reported as a wedged doorway — the "go and fix a prefab that was never
            // wrong" answer this whole family exists to avoid.
            // An UNFORCED fill still walls the leg, and the author still has to be
            // told which write did it. It is asked before the generic seam answer
            // for exactly the reason every code in this family exists: `sealed` is
            // empty here only because the seal came from a beat nobody has to play,
            // and "wedged doorway seam" would send someone to fix a prefab that is
            // perfectly correct. The blocking half of an unforced write is credited
            // in full — it is only the footing half that is withheld.
            let gate_hint = if sealed.is_empty() && has_unforced {
                "a runtime fill fired from a beat the party is NOT forced to play — a `close-gate` \
                 or `fill-region` in a trap payload, a shop offer, a death bundle or a shortcut's \
                 far side — has walled a region on/before this leg. An unforced write still SEALS \
                 for the proof (the delve must survive it) even though the footing it would lay is \
                 not credited (`DW0546`), so this wall is real. Move the fill off the forced path, \
                 fire it later, or clear it before this leg — do NOT delete the proof."
            } else if !flooded.is_empty() && sealed.is_empty() {
                "a runtime region write has filled a box with FLUID on/before this leg, and it \
                 blocks the forced path. Water and lava are impassable to a walker, so a filled \
                 box is a wall whatever its block. Move the box off the forced path, fire the \
                 fill later, or clear it before this leg — do NOT delete the proof."
            } else if sealed.is_empty() {
                "this is a wedged doorway seam, a void gap in the assembled layout, or an \
                 unbroken 1.5-tall barrier (fence/wall) ring — a walking player can neither pass \
                 through nor stand on top of a fence, so a pen needs a fence-gate opening (or, if \
                 the jump is intended, a missing inter-area transport)."
            } else {
                "a `close-gate` has sealed a gate region on/before this leg (a point of no \
                 return), and the forced path must re-cross it. Reopen it with `open-gate` \
                 before this leg, route the forced path so it does not re-cross the sealed gate, \
                 or fire the `close-gate` later — do NOT delete the proof."
            };
            return Err(Failure {
                code: DW_CRITICAL_UNROUTABLE,
                message: format!(
                    "critical path: the player cannot walk from {from:?} (floor {start:?}) to \
                     {to:?} (floor {goal:?}) over the assembled geometry — no collision-free \
                     path. A same-area leg must be walkable end to end; {gate_hint}"
                ),
            });
        }
    }
    Ok(())
}
