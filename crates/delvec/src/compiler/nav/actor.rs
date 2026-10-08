/// The declared facing of an anchor an **actor** walks to, as a yaw — the
/// counterpart of [`anchor_facing_yaw`], resolved the way an actor's anchors are
/// resolved everywhere else in this module: globally, first match, because an
/// actor carries no area. It mirrors [`actor_anchor_pos`]'s scan exactly, so the
/// facing and the position can never be read off two different anchors.
fn actor_anchor_facing_yaw(plan: &Plan, anchor: &str) -> Option<i32> {
    for ((_, name), resolved) in &plan.anchors {
        if name == anchor {
            return match resolved {
                ResolvedAnchor::Point { facing, .. } => facing
                    .as_deref()
                    .map(|f| crate::compiler::emit::facing_yaw(Some(f))),
                ResolvedAnchor::Gate { .. } => None,
            };
        }
    }
    None
}

/// A planned `move-actor` (spec-0014): resolved endpoints, the per-tick waypoint
/// polyline the emitter teleports the puppet along, and a yaw per waypoint tangent
/// to the path (a wrong yaw moonwalks). `ticks() + 1` entries.
#[derive(Debug, Clone)]
pub struct ActorMovePlan {
    /// The moving actor id (`actor/…`).
    pub actor: String,
    /// The destination mark (spec-0066): anchor and offset.
    pub to: delvewright_dsl::Mark,
    /// The integer target cell (feet), for the arrival assertion.
    pub target: [i32; 3],
    /// The A* **cell** path this leg walks, start to target inclusive — see
    /// [`MovePlan::cells`].
    pub cells: Vec<[i32; 3]>,
    /// Per-tick world positions along the walked path.
    pub waypoints: Vec<[f64; 3]>,
    /// Per-waypoint yaw (degrees), tangent to the path (facing the next step).
    pub yaws: Vec<i32>,
    /// The branch-gate component of this driver's content key ([`gate_key`]);
    /// empty for an unconditional walk.
    pub gate_key: String,
}

impl ActorMovePlan {
    /// The final tick index (`waypoints.len() - 1`).
    pub fn ticks(&self) -> usize {
        self.waypoints.len().saturating_sub(1)
    }
}

/// The stage-5 actor with this id, if declared.
fn actor_of<'a>(plan: &'a Plan, actor_id: &str) -> Option<&'a delvewright_dsl::Actor> {
    plan.campaign
        .quests
        .content
        .actors
        .iter()
        .find(|a| a.id.as_str() == actor_id)
}

/// Resolve an anchor name to a world point by scanning every area (first match) —
/// actors carry no area, so their anchors resolve globally like `open-gate`.
///
/// **First match is only an answer while one area provides the name.** A gate
/// verb whose anchor two areas provide is refused (`DW0857`), because the answer
/// there is whichever area id sorts first and nothing at the call site can tell
/// that from the right one. No such refusal guards an ACTOR's anchor yet, and a
/// released campaign already carries a point anchor two of its areas provide —
/// so widening it is a version-fenced obligation rather than a repair.
fn actor_anchor_pos(plan: &Plan, anchor: &str) -> Option<[i32; 3]> {
    for ((_, name), resolved) in &plan.anchors {
        if name == anchor {
            return Some(match resolved {
                ResolvedAnchor::Point { pos, .. } => *pos,
                ResolvedAnchor::Gate { from, .. } => *from,
            });
        }
    }
    None
}

/// Plan every `move-actor` into a walked-path [`ActorMovePlan`] over the actor's
/// footprint, deduped by `(actor, to)` in first-seen order. `DW0325` when a
/// move is unroutable (names actor, leg, first blocked cell). Each actor's
/// successive moves **chain** — first leg from the declared spawn anchor, every
/// later leg from the previous leg's target (round-6 fix; see the loop comment).
/// Two moves sharing `(actor, to)` still share one content-keyed driver,
/// planned from the first occurrence's origin (documented limitation).
///
/// Use-gate cells are walkable edges for a scripted puppet walk, exactly as for
/// `move-npc` (see [`plan_moves`]): the island ram's pen→mouth leg crosses the pen
/// gate the player has just opened — through the threshold, no longer over the
/// fence-top the full-solid model wrongly proved.
///
/// **Timeline gates (round 8).** Each walk is planned over the world with the
/// gates its own timeline already sealed forced solid ([`crate::compiler::timeline`]), so a
/// legal way around a shut gate is found when one exists, and `DW0410` is raised
/// only when none does. A deduped repeat occurrence re-verifies the *already
/// planned* path against its own timeline's seals — that is the path the shared
/// driver will actually walk.
pub fn plan_actor_moves(plan: &Plan, world: &World) -> Result<Vec<ActorMovePlan>, Failure> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<(String, String, String)> = BTreeSet::new();
    // Chained origins (round-6, live-server proven): a SECOND consecutive
    // `move-actor` on the same actor must start from the actor's CURRENT staged
    // location — the previous move's (snapped) target — not its declared spawn
    // anchor. Planning every leg from the declared anchor degenerated the
    // island's t=260 mouth→fire-pit walk into a single-waypoint instant teleport
    // (start == declared anchor == target), so the giant snapped instead of
    // walking on camera. Keyed by actor id, in campaign effect order (the same
    // deterministic order the dedup uses).
    // Branch-aware, exactly as `plan_moves`: a
    // puppet leg inherits only an origin its own branch produced. The-wake's bier
    // walked to the tide line from the GROUND branch's grave for the same reason
    // the island's Eurylochus walked from the beach.
    let mut history: StagingHistory = StagingHistory::new();
    // The cell route planned for each `(actor, to)` driver, so a deduped
    // repeat occurrence can be re-checked against its own timeline's seals.
    let mut planned: BTreeMap<(String, String, String), Vec<[i32; 3]>> = BTreeMap::new();
    // The yaw each planned driver ends on, so a deduped repeat chains it forward.
    let mut planned_end_yaw: BTreeMap<(String, String, String), i32> = BTreeMap::new();
    // The origin each driver was planned from + the branch that planned it, for
    // `DW0488`.
    let mut planned_origin: BTreeMap<(String, String, String), ([i32; 3], BranchGate)> =
        BTreeMap::new();
    let mut cache = SealCache::default();
    for (eff, seal) in crate::compiler::timeline::walk(plan) {
        let Verb::MoveActor {
            actor, to, speed, ..
        } = &eff.verb
        else {
            continue;
        };
        let gate = BranchGate::of(eff);
        // The world this walk actually happens in: gates this timeline already
        // shut are solid. Empty seal ⇒ the base world, unchanged.
        let leg_world: &World = match cache.index_of(world, &seal) {
            Some(i) => &cache.worlds[i],
            None => world,
        };
        let a = actor_of(plan, actor.as_str()).ok_or_else(|| Failure {
            code: DW_ACTOR_UNROUTABLE,
            message: format!(
                "move-actor: unknown actor `{}` — declare it in the stage-5 `actors` list",
                actor.as_str()
            ),
        })?;
        let fp = entity_footprint(&a.entity);
        // The destination is a mark (spec-0066): the snap starts from its cell.
        let to_mark = to.display();
        let dest = actor_anchor_pos(plan, to.anchor.as_str())
            .map(|p| to.cell(p))
            .ok_or_else(|| Failure {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "move-actor: destination anchor `{}` for actor `{}` did not resolve to a \
                     world position — use a `to.anchor` some area's prefab provides",
                    to.anchor.as_str(),
                    actor.as_str()
                ),
            })?;
        let target = leg_world
            .snap_standable_fp(dest, SNAP_RADIUS, &fp)
            .ok_or_else(|| Failure {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "move-actor: no cell the `{}` footprint can stand on near destination \
                     `{}` {dest:?} for actor `{}` — the mark is walled in, too low a ceiling for \
                     this mob, or over void",
                    a.entity,
                    to_mark,
                    actor.as_str()
                ),
            })?;
        let gkey = gate.key();
        let key = (actor.as_str().to_string(), to_mark.clone(), gkey.clone());
        if !seen.insert(key.clone()) {
            // Deduped: this occurrence shares the first occurrence's content-keyed
            // driver, so the path it walks is the one already planned. It still has
            // its OWN timeline, and a gate shut in *this* timeline would send that
            // shared path through solid blocks — so re-check the planned route
            // against these seals rather than waving the repeat through (DW0410).
            if !seal.is_empty()
                && let Some(cells) = planned.get(&key)
            {
                let sealed = seal_cells(&seal);
                if cells.iter().any(|c| sealed.contains(c)) {
                    return Err(gate_timeline_error(
                        "move-actor",
                        actor.as_str(),
                        to_mark.as_str(),
                        cells[0],
                        target,
                        &seal,
                    ));
                }
            }
            // Shared driver, so this occurrence starts at the origin the first
            // one planned — correct only if this branch leaves the puppet there.
            if let Some((planned_from, planned_gate)) = planned_origin.get(&key) {
                let here = chained_staging(&history, actor.as_str(), &gate).map(|s| s.pos);
                if let Some(here) = here
                    && here != *planned_from
                {
                    return Err(shared_origin_error(
                        "move-actor",
                        actor.as_str(),
                        to_mark.as_str(),
                        *planned_from,
                        planned_gate,
                        here,
                        &gate,
                    ));
                }
            }
            // The walk still ends here, so the actor's next leg chains from this
            // target — and from the facing the shared driver leaves the puppet in.
            record_staging(
                &mut history,
                actor.as_str(),
                gate,
                target,
                planned_end_yaw.get(&key).copied(),
            );
            continue;
        }
        let prior = chained_staging(&history, actor.as_str(), &gate);
        let start = match prior.map(|s| s.pos) {
            Some(pos) => pos,
            None => {
                let start_anchor = actor_anchor_pos(plan, a.anchor.as_str())
                    .map(|p| delvewright_dsl::offset_cell(p, a.offset))
                    .ok_or_else(|| Failure {
                        code: DW_ACTOR_UNROUTABLE,
                        message: format!(
                            "move-actor: actor `{}` spawn anchor `{}` did not resolve to a world \
                             position — use a spawn `anchor` some area's prefab provides",
                            actor.as_str(),
                            a.anchor.as_str()
                        ),
                    })?;
                leg_world
                    .snap_standable_fp(start_anchor, SNAP_RADIUS, &fp)
                    .unwrap_or(start_anchor)
            }
        };
        let cells = match leg_world.find_path_fp(start, target, &fp) {
            Some(cells) => cells,
            // Unroutable in the timeline-correct world. Which diagnostic depends on
            // *why*: if the open world routes it, the campaign's own `close-gate` is
            // what makes it impossible (DW0410) — otherwise the geometry simply does
            // not connect, which is the long-standing DW0325.
            None if !seal.is_empty() && world.find_path_fp(start, target, &fp).is_some() => {
                return Err(gate_timeline_error(
                    "move-actor",
                    actor.as_str(),
                    to_mark.as_str(),
                    start,
                    target,
                    &seal,
                ));
            }
            None if leg_world.has_furniture()
                && let Some(over) = leg_world
                    .without_furniture()
                    .find_path_fp(start, target, &fp) =>
            {
                return Err(furniture_route_failure(
                    &format!("move-actor `{}`", actor.as_str()),
                    &format!("{start:?}"),
                    &format!("`{to_mark}` (floor {target:?})"),
                    &leg_world.furniture_over_fp(&over, &fp),
                ));
            }
            None => {
                let blocked = first_blocked_fp(leg_world, start, target, &fp);
                return Err(Failure {
                    code: DW_ACTOR_UNROUTABLE,
                    message: format!(
                        "move-actor: actor `{}` ({}) cannot walk the leg {start:?} (last staged \
                         location; spawn anchor `{}`) → `{}` {target:?} — no collision-free path \
                         for its footprint over the assembled geometry (first blocked cell \
                         ~{blocked:?}). Route the move within one connected area, widen the \
                         corridor/ceiling for this mob, or split it into shorter reachable hops",
                        actor.as_str(),
                        a.entity,
                        a.anchor.as_str(),
                        to_mark.as_str(),
                    ),
                });
            }
        };
        planned.insert(key.clone(), cells.clone());
        planned_origin.insert(key.clone(), (start, gate.clone()));
        // The rendered motion is bounded by the volume the proof proved, so the
        // hop shape uses the SAME hitbox this leg was routed under, never the
        // entity's true size.
        let (body_w, _) = entity_dims(&actor_body_entity(a));
        // A puppet crosses open ground the same way a villager does — smoothing
        // belongs to a walked body, not to the verb that first needed it. Swept at
        // the footprint THIS leg was routed under, which for an actor is its own.
        let walked = smooth_walk(leg_world, &cells, &fp, body_w);
        let (waypoints, exact) = resample_body(&walked, speed.unwrap_or(DEFAULT_SPEED), body_w);
        // Seed: the facing the puppet already has — the exit yaw of the previous
        // leg **on this branch**, else the actor's declared spawn `facing`
        // (`emit::actor_facing_yaw`).
        let seed = prior
            .and_then(|s| s.yaw)
            .unwrap_or_else(|| crate::compiler::emit::facing_yaw(a.facing.map(|f| f.token())));
        let mut yaws = yaws_along(&exact, seed);
        // A puppet takes its arrival turn for the same reason a villager does —
        // it is a walked body, and the verb it was moved by is not what decides
        // which way it ends up looking.
        apply_arrival_yaw(&mut yaws, actor_anchor_facing_yaw(plan, to.anchor.as_str()));
        let end_yaw = yaws.last().copied().unwrap_or(seed);
        record_staging(&mut history, actor.as_str(), gate, target, Some(end_yaw));
        planned_end_yaw.insert(key, end_yaw);
        out.push(ActorMovePlan {
            actor: actor.as_str().to_string(),
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

/// Verify every declared actor's spawn anchor resolves to a world position (the
/// puppet has somewhere to spawn). `DW0325` when it does not. Needs no `World` (a
/// spawn is a summon, not a walk), so it runs even for spawn-only campaigns.
pub fn check_actor_placement(plan: &Plan) -> Result<(), Failure> {
    for a in &plan.campaign.quests.content.actors {
        if actor_anchor_pos(plan, a.anchor.as_str()).is_none() {
            return Err(Failure {
                code: DW_ACTOR_UNROUTABLE,
                message: format!(
                    "actor `{}` spawn anchor `{}` did not resolve to a world position — use an \
                     `anchor` some area's prefab provides",
                    a.id.as_str(),
                    a.anchor.as_str()
                ),
            });
        }
    }
    Ok(())
}
