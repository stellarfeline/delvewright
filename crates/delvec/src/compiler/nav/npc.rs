//! The `move-npc` walk: an NPC's staged legs over the assembled world
//! (`DW0307`).

use super::*;
use crate::compiler::failure::Failure;
use crate::compiler::plan::{BodyScope, BodyStation, Plan, ResolvedAnchor, body_station};
use delvewright_dsl::Verb;
/// [`PLAYER_WIDTH`] is imported for a different question than the step rule's
/// constants ([`World::neighbors_fp`]) and is deliberately not a
/// fourth arm of the rule: it is the fallback hitbox [`npc_render_width`] hands
/// [`step_vertices`], which shapes a step the router has **already** permitted
/// and never re-decides whether it may be taken.
use delvewright_dsl::metrics::PLAYER_WIDTH;
use std::collections::{BTreeMap, BTreeSet};

/// A planned `move-npc`: the resolved endpoints plus the per-tick waypoint
/// polyline the emitter teleports the NPC body + interaction hitbox along.
/// `waypoints[0]` is the origin and `waypoints.last()` is exactly the integer
/// target cell; there are `ticks() + 1` entries.
#[derive(Debug, Clone)]
pub struct MovePlan {
    /// The moving NPC id (`npc/…`).
    pub npc: String,
    /// The destination mark (spec-0066): anchor and offset.
    pub to: delvewright_dsl::Mark,
    /// The integer target cell (feet), for the arrival assertion.
    pub target: [i32; 3],
    /// The A* **cell** path this leg walks, start to target inclusive — the route
    /// before [`resample`] turns it into per-tick positions. Kept because the
    /// per-tick positions answer *where the body is* while a traversal proof
    /// ([`crate::compiler::traversal`]) must ask *what move the body made*: which cell it
    /// entered, and which cell it stepped up onto.
    pub cells: Vec<[i32; 3]>,
    /// Per-tick world positions along the walked path.
    pub waypoints: Vec<[f64; 3]>,
    /// Per-waypoint yaw (degrees), the bearing of the segment the body is walking
    /// (see [`yaws_along`]). Without it a tp'd body keeps a stale yaw and glides
    /// backwards — owner playtest, island round 13.
    pub yaws: Vec<i32>,
    /// The branch-gate component of this driver's content key ([`gate_key`]);
    /// empty for an unconditional walk.
    pub gate_key: String,
}

impl MovePlan {
    /// The final tick index (`waypoints.len() - 1`).
    pub fn ticks(&self) -> usize {
        self.waypoints.len().saturating_sub(1)
    }
}

/// Resample a cell path into per-tick waypoints at `speed` blocks/tick along the
/// polyline through the cell centres ([`cell_center`]). Guarantees the final
/// waypoint is exactly the goal cell's centre and at least one step exists.
///
/// **A vertical step is rendered as a step, never as a translation in place.**
/// See [`step_vertices`]: the rise is folded into the crossing as far as the
/// body's own hitbox allows, and a drop stays level until its footprint has
/// cleared the cell it is leaving. A straight lerp between the two cell centres
/// would instead sweep the body through the *corner* of the step block — the
/// same "inside the geometry" artifact at a stair that the centring fixes along
/// a wall.
/// The hitbox width of the body a stage-2 NPC actually wears — the one
/// [`crate::compiler::clearance`] judges and a viewer sees, which is what the step shape
/// has to be sound for. Falls back to the player's own width for an NPC the plan
/// does not carry, which is the routing footprint and so never widens anything.
fn npc_render_width(plan: &Plan, npc_id: &str) -> f64 {
    plan.campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)
        .map_or(PLAYER_WIDTH, |n| entity_dims(&npc_body_entity(n)).0)
}

/// The absolute world position of an NPC's home anchor (its spawn cell), which is
/// where a `move-npc` walk begins.
fn npc_start(plan: &Plan, npc_id: &str) -> Option<[i32; 3]> {
    let npc = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)?;
    let area = plan.npc_area(npc_id)?;
    plan.point(area, npc.anchor.as_str())
        .map(|p| delvewright_dsl::offset_cell(p, npc.offset))
}

/// Resolve a `move-npc` destination through [`body_station`], the one
/// authority for where a body stands, instead of nav's own by-name scan.
///
/// The caller's scope is [`BodyScope::Beat`], exactly as the cast ledger's
/// per-beat station declares it: the quest whose bundle fired this move
/// (`beat`, when [`crate::compiler::timeline::walk_with_beat_area`] found one) first,
/// then the NPC's own declared area (`home`), then an unambiguous crossing.
/// Before this, the fallback step scanned every area for the first name
/// match with no ambiguity check and no beat priority at all — home always
/// won even when the move's own quest names the anchor too, and two other
/// areas sharing a name settled silently by whichever sorted first.
fn move_target(
    plan: &Plan,
    npc_id: &str,
    to_anchor: &str,
    beat: Option<&str>,
) -> Result<[i32; 3], Failure> {
    let home = plan.npc_area(npc_id).unwrap_or("");
    let scope = BodyScope::Beat {
        beat: beat.unwrap_or(home),
        home,
    };
    match body_station(&plan.anchors, scope, to_anchor) {
        station @ BodyStation::At { .. } => Ok(station.pos().expect("an `At` station has a place")),
        BodyStation::Ambiguous(areas) => Err(Failure::new(
            crate::compiler::gates::DW_ANCHOR_AMBIGUOUS,
            format!(
                "move-npc: destination anchor `{to_anchor}` for NPC `{npc_id}` is a name {n} of \
                 this campaign's areas provide ({list}) — neither the quest this move fires \
                 from nor the npc's own area (`{home}`) is among them, so nothing an author can \
                 see says which building the body is walking to. {remedy}",
                n = areas.len(),
                list = areas
                    .iter()
                    .map(|a| format!("`{a}`"))
                    .collect::<Vec<_>>()
                    .join(", "),
                remedy = crate::compiler::gates::anchor_ambiguity_remedy(
                    &areas.iter().map(|a| (a.clone(), BTreeSet::new())).collect()
                ),
            ),
        )),
        BodyStation::Missing => Err(Failure::new(
            DW_MOVE_UNROUTABLE,
            format!(
                "move-npc: destination anchor `{to_anchor}` for NPC `{npc_id}` did not resolve \
                 to a world position — use a `to.anchor` that the NPC's area prefab provides"
            ),
        )),
    }
}

/// Plan every `move-npc` in the campaign into a walked-path [`MovePlan`], deduped
/// by `(npc, to)` in first-seen order. `DW0307` when a move is
/// unroutable, `DW0859` when its destination names an anchor two areas answer
/// to and neither the move's own quest nor the NPC's home settles it
/// ([`move_target`]). Each NPC's successive moves **chain**: the first leg
/// starts at the stage-2 anchor, every later leg at the previous leg's target
/// (round-6; see
/// [`plan_actor_moves`]). Two moves sharing `(npc, to)` still share one
/// content-keyed driver, planned from the first occurrence's origin (documented
/// limitation of the content key).
///
/// **Use-gate cells are walkable edges here**: routing through the
/// openable threshold is strictly more faithful than the old full-solid model,
/// which "proved" the same legs by hopping the body over a fence-top. Only
/// autonomous placement (wave seating) uses the no-gate-use view — a spawned mob
/// really cannot pass a closed gate on its own.
///
/// This edge used to be justified by "the beat's fiction controls the gate" (the
/// island ram leaves its pen only after the player has opened the pen gate to
/// reach it). **Nothing proved that fiction**, and island round 21 is what it
/// cost: the mountain pen's gate shipped `open=false` and sixteen legs walked
/// through it, in a cell the owner herself had to squeeze around. The edge is
/// still available here — a route that names the offending cell is a better
/// diagnostic than an unroutable [`DW_MOVE_UNROUTABLE`] — but
/// [`crate::compiler::traversal`] now fails the build on it (`DW0452`) unless the gate is
/// genuinely open in the world the delve ships.
pub fn plan_moves(plan: &Plan, world: &World) -> Result<Vec<MovePlan>, Failure> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    // Chained origins (round-6): each NPC's next walk starts from its LAST staged
    // location — the previous move's (snapped) target — not its declared anchor.
    // Planning every leg from the declared anchor made a second consecutive
    // `move-npc` on the same NPC degenerate (worst case start == target → a
    // single-waypoint instant teleport instead of a walk).
    //
    // The chain is **branch-aware** (island round 16): a leg only inherits an
    // origin a leg on its own branch actually produced, so a `flag/flee`-gated
    // walk can no longer hand its destination to the `flag/wait`-gated walk that
    // follows it in declaration order. See [`BranchGate`] for the defect this
    // fixes and why the rule is stated as implication rather than exclusion.
    let mut history: StagingHistory = StagingHistory::new();
    // The cell route planned for each `(npc, to)` driver, so a deduped
    // repeat occurrence can be re-checked against its own timeline's seals.
    let mut planned: BTreeMap<(String, String, String), Vec<[i32; 3]>> = BTreeMap::new();
    // The yaw each planned driver ends on, so a deduped repeat chains the same
    // facing forward as the first occurrence did.
    let mut planned_end_yaw: BTreeMap<(String, String, String), i32> = BTreeMap::new();
    // The origin each driver was planned from, and the branch of the occurrence
    // that planned it — so a deduped occurrence standing somewhere else is
    // `DW0488` instead of a silent teleport.
    let mut planned_origin: BTreeMap<(String, String, String), ([i32; 3], BranchGate)> =
        BTreeMap::new();
    let mut cache = SealCache::default();
    for (eff, seal, beat) in crate::compiler::timeline::walk_with_beat_area(plan) {
        let Verb::MoveNpc { npc, to, speed, .. } = &eff.verb else {
            continue;
        };
        let gate = BranchGate::of(eff);
        // The world this walk actually happens in: gates this timeline already
        // shut are solid. Empty seal ⇒ the base world, unchanged.
        let leg_world: &World = match cache.index_of(world, &seal) {
            Some(i) => &cache.worlds[i],
            None => world,
        };
        // The destination is a mark (spec-0066): the anchor resolves through the
        // one station authority, and the snap starts from the mark's cell.
        let to_mark = to.display();
        let anchor_pos = to.cell(move_target(plan, npc.as_str(), to.anchor.as_str(), beat)?);
        let target = leg_world
            .snap_standable(anchor_pos, SNAP_RADIUS)
            .ok_or_else(|| Failure {
                code: DW_MOVE_UNROUTABLE,
                message: format!(
                    "move-npc: no standable floor cell near destination `{}` {anchor_pos:?} \
                 for NPC `{}` — the mark is walled in or over void; place `{}` beside walkable \
                 floor the npc can stand on",
                    to_mark,
                    npc.as_str(),
                    to_mark,
                ),
            })?;
        let gkey = gate.key();
        let key = (npc.as_str().to_string(), to_mark.clone(), gkey.clone());
        if !seen.insert(key.clone()) {
            // Deduped: shares the first occurrence's driver, so it walks the
            // already-planned path — which must still be clear under THIS
            // occurrence's timeline seals (DW0410; see `plan_actor_moves`).
            if !seal.is_empty()
                && let Some(cells) = planned.get(&key)
            {
                let sealed = seal_cells(&seal);
                if cells.iter().any(|c| sealed.contains(c)) {
                    return Err(gate_timeline_error(
                        "move-npc",
                        npc.as_str(),
                        to_mark.as_str(),
                        cells[0],
                        target,
                        &seal,
                    ));
                }
            }
            // This occurrence walks the driver the FIRST occurrence planned, so
            // it starts at that driver's origin — which is only correct if this
            // occurrence's own branch leaves the body there too (`DW0488`).
            if let Some((planned_from, planned_gate)) = planned_origin.get(&key) {
                let here = chained_staging(&history, npc.as_str(), &gate).map(|s| s.pos);
                if let Some(here) = here
                    && here != *planned_from
                {
                    return Err(shared_origin_error(
                        "move-npc",
                        npc.as_str(),
                        to_mark.as_str(),
                        *planned_from,
                        planned_gate,
                        here,
                        &gate,
                    ));
                }
            }
            // The walk still ends here, so the NPC's next leg chains from this
            // target — and from the facing the shared driver leaves the body in.
            record_staging(
                &mut history,
                npc.as_str(),
                gate,
                target,
                planned_end_yaw.get(&key).copied(),
            );
            continue;
        }
        let prior = chained_staging(&history, npc.as_str(), &gate);
        let start = match prior.map(|s| s.pos) {
            Some(pos) => pos,
            None => {
                let home = npc_start(plan, npc.as_str()).ok_or_else(|| Failure {
                    code: DW_MOVE_UNROUTABLE,
                    message: format!(
                        "move-npc: NPC `{}` has no resolved home anchor to walk from — give the \
                         npc a stage-2 `anchor` that its area's prefab provides, so the walk has \
                         a start",
                        npc.as_str()
                    ),
                })?;
                // The NPC walks up to a solid affordance, not into it: snap the
                // home endpoint to the floor cell nearest the anchor.
                leg_world.snap_standable(home, SNAP_RADIUS).unwrap_or(home)
            }
        };
        let seed_yaw = prior.and_then(|s| s.yaw);
        let cells = match leg_world.find_path(start, target) {
            Some(cells) => cells,
            // Routable open, unroutable sealed ⇒ this timeline's own `close-gate`
            // is the cause (DW0410); otherwise the geometry never connected (DW0307).
            None if !seal.is_empty() && world.find_path(start, target).is_some() => {
                return Err(gate_timeline_error(
                    "move-npc",
                    npc.as_str(),
                    to_mark.as_str(),
                    start,
                    target,
                    &seal,
                ));
            }
            None if leg_world.has_furniture()
                && let Some(over) = leg_world.without_furniture().find_path(start, target) =>
            {
                return Err(furniture_route_failure(
                    &format!("move-npc `{}`", npc.as_str()),
                    &format!("{start:?}"),
                    &format!("`{to_mark}` (floor {target:?})"),
                    &leg_world.furniture_over(&over),
                ));
            }
            None => {
                return Err(Failure {
                    code: DW_MOVE_UNROUTABLE,
                    message: format!(
                        "move-npc: NPC `{}` cannot walk from its last staged location {start:?} \
                         (home anchor `{}`) to `{}` {anchor_pos:?} (floor {target:?}) — no \
                         collision-free path over the solved geometry. Route the move within one \
                         connected area (a wall/void/closed gate separates start and \
                         destination), or split it into shorter reachable hops",
                        npc.as_str(),
                        plan_npc_anchor(plan, npc.as_str()),
                        to_mark.as_str(),
                    ),
                });
            }
        };
        planned.insert(key.clone(), cells.clone());
        planned_origin.insert(key.clone(), (start, gate.clone()));
        // The body walks the string-pulled polyline, not the four-connected
        // staircase A* returned — `cells` keeps the route for the proofs that read
        // it. Routed on the player footprint (this is `find_path`), swept at the
        // width the NPC actually wears.
        let width = npc_render_width(plan, npc.as_str());
        let walked = smooth_walk(leg_world, &cells, &Footprint::player(), width);
        let (waypoints, exact) = resample_body(&walked, speed.unwrap_or(DEFAULT_SPEED), width);
        // Seed: the facing this body already has — the exit yaw of the previous
        // leg **on this branch** if this NPC has walked before, else the yaw its
        // summon gave it (the home anchor's declared facing,
        // `emit::npc_summon_commands`).
        let seed = seed_yaw.unwrap_or_else(|| npc_spawn_yaw(plan, npc.as_str()));
        // Yawed off the EXACT samples: the rounding that keeps the emitted
        // coordinates short would otherwise twitch a diagonal's bearing every tick.
        let mut yaws = yaws_along(&exact, seed);
        apply_arrival_yaw(
            &mut yaws,
            anchor_facing_yaw(plan, npc.as_str(), to.anchor.as_str()),
        );
        let end_yaw = yaws.last().copied().unwrap_or(seed);
        record_staging(&mut history, npc.as_str(), gate, target, Some(end_yaw));
        planned_end_yaw.insert(key, end_yaw);
        out.push(MovePlan {
            npc: npc.as_str().to_string(),
            to: to.clone(),
            target,
            cells,
            waypoints,
            yaws,
            gate_key: gkey,
        });
    }
    Ok(out)
}

/// The declared facing of one anchor in an NPC's own area, as a yaw — `None`
/// when that anchor declares none.
fn anchor_facing_yaw(plan: &Plan, npc_id: &str, anchor_id: &str) -> Option<i32> {
    let area = plan.npc_area(npc_id)?;
    match plan.anchors.get(&(area.to_string(), anchor_id.to_string())) {
        Some(ResolvedAnchor::Point { facing, .. }) => facing
            .as_deref()
            .map(|f| crate::compiler::emit::facing_yaw(Some(f))),
        _ => None,
    }
}

/// The yaw an NPC's summon gives it: its home anchor's declared `facing`, exactly
/// as `emit::npc_summon_commands` resolves it. The seed a walk starts from, so a
/// move that opens with no horizontal motion keeps the body's authored facing
/// instead of snapping it south.
fn npc_spawn_yaw(plan: &Plan, npc_id: &str) -> i32 {
    let facing = (|| {
        let npc = plan
            .campaign
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc_id)?;
        let area = plan.npc_area(npc_id)?;
        match plan
            .anchors
            .get(&(area.to_string(), npc.anchor.to_string()))
        {
            Some(ResolvedAnchor::Point { facing, .. }) => facing.clone(),
            _ => None,
        }
    })();
    crate::compiler::emit::facing_yaw(facing.as_deref())
}

/// The home-anchor id of an NPC, for diagnostics (or `?` if unknown).
fn plan_npc_anchor(plan: &Plan, npc_id: &str) -> String {
    plan.campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc_id)
        .map(|n| n.anchor.as_str().to_string())
        .unwrap_or_else(|| "?".to_string())
}
