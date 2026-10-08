//! The critical path and the manifest.

use super::*;

/// The scheduled tail (ticks) between firing `effs` and a `campaign-complete`
/// nested anywhere inside it — `None` when the bundle reaches none.
///
/// spec-0025 / DW0481 admit the ending at any nesting depth (the-wake schedules
/// its finale 250t into the closing `sequence`), so every consumer that waits
/// for the ending — the campaign PackTest, the harness completion window — must
/// wait out the tail the emitter itself scheduled. `sequence` steps add their
/// `at_ticks`; a `move-npc` / `move-actor` `on_arrive` adds the planned walk
/// duration. Reaction bundles (`on_respawn` / `on_rest` / `on_caught`) are
/// skipped: driving objective completions never fires them, and `DW0204` proves
/// the path's ending does not live exclusively there. Flag gates are ignored —
/// a gated ending yields an upper bound, and waiting longer can only wait,
/// never wrongly pass.
pub(super) fn campaign_complete_tail(
    effs: &[QuestEffect],
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) -> Option<u32> {
    effs.iter()
        .filter_map(|e| match &e.verb {
            Verb::CampaignComplete { .. } => Some(0),
            Verb::Sequence { steps } => steps
                .iter()
                .filter_map(|s| {
                    campaign_complete_tail(&s.effects, moves, actor_moves).map(|t| s.at_ticks + t)
                })
                .max(),
            Verb::MoveNpc {
                npc, to, on_arrive, ..
            } => campaign_complete_tail(on_arrive, moves, actor_moves).map(|t| {
                t + moves
                    .iter()
                    .find(|m| m.npc == npc.as_str() && m.to == *to)
                    .map(|m| m.ticks() as u32)
                    .unwrap_or(0)
            }),
            Verb::MoveActor {
                actor,
                to,
                on_arrive,
                ..
            } => campaign_complete_tail(on_arrive, moves, actor_moves).map(|t| {
                t + actor_moves
                    .iter()
                    .find(|m| m.actor == actor.as_str() && m.to == *to)
                    .map(|m| m.ticks() as u32)
                    .unwrap_or(0)
            }),
            _ => None,
        })
        .max()
}

/// The ending tail a driven run of `quest_ids` can schedule: the max
/// [`campaign_complete_tail`] over those quests' `on_objective_complete` bundles
/// and `on_complete`. `0` when the ending is synchronous.
pub(super) fn quests_ending_tail(
    c: &delvewright_dsl::Campaign,
    quest_ids: &BTreeSet<&str>,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) -> u32 {
    c.quests
        .content
        .quests
        .iter()
        .filter(|q| quest_ids.contains(q.id.as_str()))
        .flat_map(|q| {
            q.on_objective_complete
                .values()
                .map(|effs| effs.as_slice())
                .chain(std::iter::once(q.on_complete.as_slice()))
        })
        .filter_map(|effs| campaign_complete_tail(effs, moves, actor_moves))
        .max()
        .unwrap_or(0)
}

/// Splice a **`rest`** step into the exported critical path after the beat that
/// arms each bonfire (spec-0016 §1; bell round-3 finding, 2026-08-03).
///
/// A bonfire arms an affordance and moves nothing until the party rests — which is
/// souls-correct and was also invisible to the validation ladder: the proven path
/// walked past every bonfire without touching it, so the checkpoint never moved,
/// and a die-retry trial respawned the bot at world spawn (the beach) instead of
/// at the fire it had just walked past. The walk-back budget blew, and the run
/// judged the *campaign* for a *proof* that never performed the player loop.
///
/// Resting is the intended loop, so the proven path performs it: after the step
/// that arms bonfire `i` (its `fire_step` — the earliest tick at which a rest is
/// possible, the same index `DW0315` roots the no-stranding proof at), the path
/// gains one `rest` step choosing **rest and save**. Several bonfires armed by the
/// same beat are spliced in bonfire order, so the emission is deterministic.
///
/// This is a *path export* change only: `plan.critical_path` is untouched, so
/// every `fire_step` index, every nav proof and every other consumer sees exactly
/// what it saw before. Emits nothing for a campaign with no bonfire →
/// byte-identical.
///
/// **Step shape** (the harness contract; execution lands in a follow-up):
/// ```json
/// { "action": "rest", "bonfire": 0, "anchor": "anchor/keeper-stand",
///   "pos": [44, 65, 2], "command": "/trigger dw.rest set 2" }
/// ```
/// The bot walks to `pos`, right-clicks the `dw_bonfire_<bonfire>` interaction —
/// which is what opens the dialog and what *enables* the trigger — and then sends
/// `command`, the exact chat line the "rest and save" button runs. The click is not
/// optional: `dw.rest` is a trigger objective and is disabled until the opener
/// enables it, so a bot that only chats the command changes nothing.
///
/// `walked` is the step list the JSON was serialized from — the exported path, or
/// (spec-0025) one branch's path, whose indices are its own. See
/// [`rest_step_index`] for how a `fire_step` crosses that boundary.
pub(super) fn with_bonfire_rest_steps(
    plan: &Plan,
    walked: &[plan::Step],
    steps: Vec<Value>,
) -> Vec<Value> {
    if plan.bonfires().next().is_none() {
        return steps;
    }
    // The area a cell stands in; areas sit `plan::AREA_SPACING` apart, so the
    // horizontal box decides it.
    let area_of = |pos: &Value| -> Option<usize> {
        let c: Vec<i64> = pos.as_array()?.iter().filter_map(Value::as_i64).collect();
        let (x, z) = (*c.first()? as i32, *c.get(2)? as i32);
        plan.areas.iter().position(|a| {
            let (lo, hi) = a.bounds();
            lo[0] <= x && x <= hi[0] && lo[2] <= z && z <= hi[2]
        })
    };
    let step_area = |st: &Value| st.get("pos").and_then(area_of);
    // A rest is spliced where the party can walk to the fire: the step that arms
    // it, unless a crossing carries the party out of the fire's area right after
    // that step (a crossing is the only move between areas, so the next step then
    // stands in another area) — then the first later step after which the party
    // stands in the fire's area again. On a route that stays in one area this is
    // always the arming step.
    let rest_after: Vec<(usize, &plan::CheckpointPlan)> = plan
        .bonfires()
        .filter_map(|b| {
            let armed = rest_step_index(plan, walked, b.fire_step)?;
            let Some(home) = plan.areas.iter().position(|a| {
                let (lo, hi) = a.bounds();
                lo[0] <= b.pos[0] && b.pos[0] <= hi[0] && lo[2] <= b.pos[2] && b.pos[2] <= hi[2]
            }) else {
                return Some((armed, b));
            };
            // Where the party stands after step `i`: the last step at or before
            // it that names a position; a step that names none (a class pick,
            // the completion assert) leaves it where it was.
            let stands_home = |i: usize| {
                let here = steps[..=i].iter().rev().find_map(step_area);
                let next = steps[i + 1..].iter().find_map(step_area);
                here == Some(home) && next.is_none_or(|a| a == home)
            };
            let at = (armed..steps.len()).find(|&i| stands_home(i))?;
            Some((at, b))
        })
        .collect();
    let mut out: Vec<Value> = Vec::with_capacity(steps.len());
    for (i, step) in steps.into_iter().enumerate() {
        out.push(step);
        for bf in rest_after
            .iter()
            .filter(|(at, _)| *at == i)
            .map(|(_, b)| *b)
        {
            let mut rest = json!({
                "action": "rest",
                "bonfire": bf.index,
                "anchor": bf.anchor,
                "pos": bf.pos,
                "command": "/trigger dw.rest set 2"
            });
            // A rest plays the bonfire's `on_rest`: every cutscene on its
            // timeline is waited out after the rest (`compiler::hold`).
            if let Some(secs) = crate::compiler::hold::rest_hold(&bf.on_respawn)
                && secs > 0
            {
                rest["cutscene_seconds"] = json!(secs);
            }
            out.push(rest);
        }
    }
    out
}

/// Where bonfire `fire_step` — an index into the EXPORTED path — lands on `walked`.
///
/// A per-branch path (spec-0025) is a different sequence of the same steps, so the
/// index cannot be carried across: it is translated through the **objective** the
/// firing beat names, because a fire is armed by a beat, not by a position. A beat
/// that does not happen on this branch arms nothing there, and the branch path
/// carries no rest step for it. On the exported path the translation is the
/// identity (an objective appears at exactly one step), so this emits byte-for-byte
/// what it emitted before.
pub(super) fn rest_step_index(
    plan: &Plan,
    walked: &[plan::Step],
    fire_step: usize,
) -> Option<usize> {
    match plan
        .critical_path
        .get(fire_step)
        .and_then(plan::Step::objective)
    {
        // `fire_step: 0` is the class-select / conservative "before everything"
        // index (see `Plan::gate_fired_before`); it precedes every path the same way.
        None => (fire_step < walked.len()).then_some(fire_step),
        Some(obj) => walked.iter().position(|s| s.objective() == Some(obj)),
    }
}

pub(super) fn emit_critical_path(
    plan: &Plan,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    routes: &[crate::compiler::nav::LegRoute],
) -> Value {
    let en_route = crate::compiler::hold::en_route_holds(plan, &plan.critical_path, routes);
    critical_path_json(
        plan,
        &plan.critical_path,
        &plan.critical_path_transport,
        &plan.critical_path_sneak,
        CutsceneHolds {
            after: &plan.critical_path_cutscene,
            en_route: &en_route,
        },
        moves,
        actor_moves,
    )
}

/// Serialize a step list in the `critical-path.json` contract (format 2).
///
/// One serializer for the exported path and for every spec-0025 per-branch path:
/// a branch run must consume a contract the harness already parses, so the branch
/// tier cannot drift into a second, less-tested shape.
/// The two per-step cutscene numbers a path exports (`compiler::hold`):
/// `cutscene_seconds` and `en_route_cutscene_seconds`, each aligned 1:1 with
/// the path's steps.
#[derive(Clone, Copy)]
pub(super) struct CutsceneHolds<'a> {
    pub(super) after: &'a [Option<u32>],
    pub(super) en_route: &'a [Option<u32>],
}

pub(super) fn critical_path_json(
    plan: &Plan,
    walked: &[plan::Step],
    transports: &[Option<[i32; 3]>],
    sneak: &[bool],
    holds: CutsceneHolds<'_>,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) -> Value {
    // Scheduled-ending tail for THIS path's quests: exported on the
    // terminal `assert-complete` step as `ending_tail_ticks`, so the harness
    // completion window covers a `sequence`-scheduled finale (the-wake: 250t)
    // exactly as it already covers `cutscene_seconds`. Omitted when 0, keeping
    // every synchronous-ending path byte-identical.
    let path_quests: BTreeSet<&str> = {
        let objs: BTreeSet<&str> = walked.iter().filter_map(plan::Step::objective).collect();
        plan.campaign
            .quests
            .content
            .quests
            .iter()
            .filter(|q| q.objectives.iter().any(|o| objs.contains(o.id().as_str())))
            .map(|q| q.id.as_str())
            .collect()
    };
    let ending_tail = quests_ending_tail(plan.campaign, &path_quests, moves, actor_moves);
    let steps: Vec<Value> = walked
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let transport = &transports[i];
            let mut step = match s {
                Step::SelectClass { class_id, command } => json!({
                    "action": "select-class", "class": class_id, "command": command
                }),
                Step::TalkTo { objective_id, npc_id, pos, command } => json!({
                    "action": "talk-to", "objective": objective_id, "npc": npc_id,
                    "pos": pos, "command": command
                }),
                // `completion` is the volume the server adjudicates in, not a
                // second description of it: it comes off the same
                // `plan::reach_completion` call the tick line is formatted from.
                // `radius` stays beside it as the AUTHORED number, which is a
                // different fact and is what a report cites.
                Step::Reach { objective_id, anchor_id, pos, radius, completion } => json!({
                    "action": "reach", "objective": objective_id, "anchor": anchor_id,
                    "pos": pos, "radius": radius, "completion": completion.to_json()
                }),
                Step::Kill { objective_id, wave_id, pos, tag, count } => json!({
                    "action": "kill", "objective": objective_id, "wave": wave_id,
                    "pos": pos, "tag": tag, "count": count
                }),
                Step::Collect { objective_id, item, count, pos, dropped } => {
                    let mut v = json!({
                        "action": "collect", "objective": objective_id, "item": item,
                        "count": count, "pos": pos
                    });
                    // v0.9: present only on a drop-gated collect, so every
                    // pre-0.9 campaign's `critical-path.json` is byte-identical.
                    if let Some(w) = dropped
                        && let Some(obj) = v.as_object_mut()
                    {
                        obj.insert("dropped_by".to_string(), json!(w));
                    }
                    v
                }
                Step::Interact { objective_id, anchor_id, pos, command, requires_item, block } => json!({
                    "action": "interact", "objective": objective_id, "anchor": anchor_id,
                    "pos": pos, "command": command, "requires_item": requires_item,
                    "block": block
                }),
                // A path act that proves no objective: it passes on the trigger's
                // own fired marker (`[dw:complete <campaign> trigger/<id>]`,
                // broadcast from its bundle), never on the click landing. `anchor`
                // / `npc` / `range` are present exactly when the kind has one.
                Step::Trigger { trigger_id, on, anchor_id, npc_id, assembly_id, pos, range, stand, block } => {
                    let mut v = json!({
                        "action": "trigger", "trigger": trigger_id, "on": on, "pos": pos
                    });
                    if let Some(obj) = v.as_object_mut() {
                        // spec-0083 §4: a trigger that carries the party says
                        // where to stand to be carried; its `transport` is the
                        // link's `to`, written below like every carried step's.
                        if let Some(c) = stand {
                            obj.insert("stand".to_string(), json!(c));
                        }
                        if let Some(a) = anchor_id {
                            obj.insert("anchor".to_string(), json!(a));
                        }
                        if let Some(n) = npc_id {
                            obj.insert("npc".to_string(), json!(n));
                        }
                        if let Some(m) = assembly_id {
                            obj.insert("assembly".to_string(), json!(m));
                        }
                        if let Some(r) = range {
                            obj.insert("range".to_string(), json!(r));
                        }
                        if let Some(b) = block {
                            obj.insert("block".to_string(), json!(b));
                        }
                    }
                    v
                }
                // spec-0086 §6: a loop exercised on the path. `transport` is the
                // landing, written by the shared marker below from the same
                // per-step transport the route proof reads.
                Step::Loop { loop_id, pos, cross, offset, times, transport } => json!({
                    "action": "loop", "loop": loop_id, "pos": pos, "cross": cross,
                    "offset": offset, "times": times, "transport": transport
                }),
                Step::AssertComplete { objective, value } => {
                    let mut v = json!({
                        "action": "assert-complete", "scoreboard": { "objective": objective, "value": value }
                    });
                    if ending_tail > 0
                        && let Some(obj) = v.as_object_mut()
                    {
                        obj.insert("ending_tail_ticks".to_string(), json!(ending_tail));
                    }
                    v
                }
            };
            // gap 8: mark a step whose completion teleports the player to another
            // area with the absolute destination, so the harness waits for the
            // position discontinuity before starting the next step.
            if let (Some(pos), Some(obj)) = (transport, step.as_object_mut()) {
                obj.insert("transport".to_string(), json!(pos));
            }
            // A reach the previous step's landing puts the party inside completes
            // on that landing (`plan::completed_on_landing`); present only when
            // true, so every path without one is byte-identical.
            if plan::completed_on_landing(walked, transports, i)
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert("completed_on_landing".to_string(), json!(true));
            }
            // DSL v0.4 harness hints. `sneak` is emitted ONLY when true (absent =
            // false, per the harness contract). `cutscene_seconds` is a positive
            // integer on the step whose completion triggers the cutscene.
            if sneak[i]
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert("sneak".to_string(), json!(true));
            }
            if let Some(secs) = holds.after[i]
                && secs > 0
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert("cutscene_seconds".to_string(), json!(secs));
            }
            // The longest a cutscene fired by something this step's walk passes
            // can hold the party (`compiler::hold::en_route_holds`); present only
            // when one can.
            if let Some(secs) = holds.en_route.get(i).copied().flatten()
                && secs > 0
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert("en_route_cutscene_seconds".to_string(), json!(secs));
            }
            step
        })
        .collect();
    let steps = with_bonfire_rest_steps(plan, walked, steps);
    json!({
        // Campaign-derived (not the compiler's max supported version): a v0.2
        // campaign emits a v0.2 critical path, a v0.3 campaign a v0.3 one.
        "version": plan.campaign.world.dsl_version,
        // The bot-contract version, independent of the DSL version: `2` = every
        // objective-bearing step names the objective it proves, and completion is
        // proved by the anchored marker channel. The harness refuses anything else.
        "format_version": plan::CRITICAL_PATH_FORMAT_VERSION,
        "campaign_id": plan.namespace,
        // Format 4: the delve's cast statement — which entity kinds are never a
        // combat target. It rides on the path rather than on `combat-plan.json`
        // because it is a fact about the WORLD the bot walks, not about a fight:
        // a delve with NPCs and no combat ships no combat plan at all, and its
        // bot must still know not to swing back at a quest-giver when a fall
        // takes its health. See `combat::non_combatants`.
        "non_combatants": crate::compiler::combat::non_combatants_json(plan.campaign),
        "steps": steps
    })
}

/// `manifest.json` — the compiler's SHA-256 index over what it READ and what it
/// WROTE, plus the versions that decide how the one becomes the other.
///
/// Every value here is a function of the arguments this build was given. It used
/// to carry one that was not: `content_sha`, resolved by walking up from
/// `std::env::current_dir()` for a `versions.toml` and reading its
/// `[content].sha`. That made the manifest a function of where the operator was
/// standing — the same campaign, the same prefabs and the same seed produced a
/// different `manifest.json` from the engine checkout than from anywhere else —
/// which is exactly the ambient state ADR-0006 and `CLAUDE.md`'s forbidden zone
/// rule out, and the one flavour of it the double-build gate structurally cannot
/// see, because both of its builds share a working directory. It is gone rather
/// than repaired: a git revision is a property of a checkout, never of the bytes
/// the compiler was handed, so there was no honest value for the compiler to
/// compute. The question it pretended to answer — which content commit built
/// this delve — is answered by the party that actually knows the revision, in
/// the shipped image's `org.opencontainers.image.revision` and
/// `…delvewright.campaign-commit` labels.
pub(super) fn emit_manifest(
    plan: &Plan,
    input_bytes: &BTreeMap<String, Vec<u8>>,
    out: &BuildOutput,
    language: Option<&str>,
    resource_pack: Option<&(String, bool)>,
) -> Value {
    let inputs: BTreeMap<String, String> = input_bytes
        .iter()
        .map(|(k, v)| (k.clone(), sha256_hex(v)))
        .collect();
    let outputs: BTreeMap<String, String> = out
        .iter()
        .map(|(k, v)| (k.clone(), sha256_hex(v)))
        .collect();
    let mut manifest = json!({
        "campaign_id": plan.namespace,
        "delvec_version": DELVEC_VERSION,
        "dsl_version": plan.campaign.world.dsl_version,
        "mc_version": MC_VERSION,
        "inputs": inputs,
        "outputs": outputs
    });
    // Record the build language ONLY for a non-canonical build. English is the
    // implicit canonical language, so an `en` build's manifest is byte-identical to
    // a pre-i18n one (preserving the determinism regression for all campaigns that
    // do not localize).
    if let Some(lang) = language
        && lang != delvewright_dsl::CANONICAL_LANG
    {
        manifest
            .as_object_mut()
            .expect("manifest is a JSON object")
            .insert("language".to_string(), Value::String(lang.to_string()));
    }
    // Record the NPC-skin resource-pack SHA-1 (spec-0009: the pack bytes — and so
    // this hash — are part of the byte-identity contract). Absent for a campaign
    // with no skinned NPCs, keeping such builds byte-identical.
    //
    // Beside it, `resource_pack_overrides_vanilla` (spec-0084 §4.2): whether the
    // pack carries an `assets/minecraft/` entry. Written once, here, from the
    // archive paths the pack was built from, and read by every host-side script
    // — never re-derived from the zip.
    if let Some((sha1, overrides)) = resource_pack {
        let m = manifest.as_object_mut().expect("manifest is a JSON object");
        m.insert(
            "resource_pack_sha1".to_string(),
            Value::String(sha1.to_string()),
        );
        m.insert(
            "resource_pack_overrides_vanilla".to_string(),
            Value::Bool(*overrides),
        );
    }
    manifest
}
