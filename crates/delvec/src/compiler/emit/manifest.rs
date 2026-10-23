//! The critical path and the manifest.

use super::*;

/// **The DW codes the engine that wrote this manifest carries**: every code its
/// registry declares, sorted, once each. A ledger row's `dw` carrier is judged
/// against the engine that BUILT a tree (`tools/creator/staging-gate.py`), and
/// `delvec_version` cannot answer that — every unreleased revision states the
/// same version — so the build states the set itself. A function of the
/// binary, never of the checkout or the clock.
pub(super) fn dw_codes() -> Vec<&'static str> {
    let codes: BTreeSet<&'static str> = delvewright_dsl::diagnostic::declared()
        .iter()
        .map(|d| d.code)
        .collect();
    codes.into_iter().collect()
}

/// The scheduled tail (ticks) between firing `effs` and a `campaign-complete`
/// nested anywhere inside it — `None` when the bundle reaches none.
///
/// spec-0025 / DW0481 admit the ending at any nesting depth (the-wake schedules
/// its finale 250t into the closing `sequence`), so every consumer that waits
/// for the ending — the campaign PackTest, the harness completion window — must
/// wait out the tail the emitter itself scheduled. `sequence` steps add their
/// `at_ticks`; a `move-npc` / `move-actor` `on_arrive` adds the planned walk
/// duration; a cutscene's `then` adds the cutscene's length plus one tick
/// (spec-0108 §3.6). Reaction bundles (`on_respawn` / `on_rest` / `on_caught`)
/// are skipped: driving objective completions never fires them, and `DW0204`
/// proves the path's ending does not live exclusively there. Flag gates are
/// ignored — a gated ending yields an upper bound, and waiting longer can only
/// wait, never wrongly pass.
///
/// **The aftermath is waited out too** (spec-0108 §3.6, §6.3): where the bundle
/// reaches an ending, the tail is the later of the ending and the last effect
/// of the bundle that changes the world ([`Verb::changes_world`]), so a run
/// that asserts the world the ending left reads it after the last change landed.
pub(super) fn campaign_complete_tail(
    effs: &[QuestEffect],
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
) -> Option<u32> {
    let ending = scheduled_tail(effs, moves, actor_moves, &|e| {
        matches!(e.verb, Verb::CampaignComplete { .. })
    })?;
    let aftermath =
        scheduled_tail(effs, moves, actor_moves, &|e| e.verb.changes_world()).unwrap_or(0);
    Some(ending.max(aftermath))
}

/// The latest tick, counted from firing `effs`, at which an effect `pick`
/// accepts runs — along the bundle's own schedule (`sequence` steps, `on_arrive`
/// walks, a cutscene's `then`), reaction bundles skipped. `None` when nothing
/// on the schedule is picked.
pub(super) fn scheduled_tail(
    effs: &[QuestEffect],
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    pick: &dyn Fn(&QuestEffect) -> bool,
) -> Option<u32> {
    effs.iter()
        .filter_map(|e| {
            let own = pick(e).then_some(0);
            let nested = match &e.verb {
                Verb::Sequence { steps } => steps
                    .iter()
                    .filter_map(|s| {
                        scheduled_tail(&s.effects, moves, actor_moves, pick).map(|t| s.at_ticks + t)
                    })
                    .max(),
                Verb::MoveNpc {
                    npc, to, on_arrive, ..
                } => scheduled_tail(on_arrive, moves, actor_moves, pick).map(|t| {
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
                } => scheduled_tail(on_arrive, moves, actor_moves, pick).map(|t| {
                    t + actor_moves
                        .iter()
                        .find(|m| m.actor == actor.as_str() && m.to == *to)
                        .map(|m| m.ticks() as u32)
                        .unwrap_or(0)
                }),
                Verb::Cutscene { then, .. } => {
                    let end = crate::compiler::link::cutscene_end_offset(e).unwrap_or(0);
                    scheduled_tail(then, moves, actor_moves, pick).map(|t| end + 1 + t)
                }
                _ => None,
            };
            match (own, nested) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (a, b) => a.or(b),
            }
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

/// Lay a path's steps out as its exported `critical-path.json` carries them
/// ([`Plan::exported_layout`]): each step of the walked path, a **`rest`** step
/// where the party rests at each bonfire (spec-0016 §1; [`Plan::rest_after`]),
/// and each witnessed assembly's witness steps (spec-0082 §5.4).
///
/// A bonfire arms an affordance and moves nothing until the party rests — which is
/// souls-correct and was also invisible to the validation ladder: the proven path
/// walked past every bonfire without touching it, so the checkpoint never moved,
/// and a die-retry trial respawned the bot at world spawn instead of at the fire
/// it had just walked past. Resting is the intended loop, so the proven path
/// performs it, choosing **rest and save**.
///
/// This is a *path export* change only: `plan.critical_path` is untouched, so
/// every `fire_step` index, every nav proof and every other consumer sees exactly
/// what it saw before. A campaign with no bonfire and no witness is laid out as
/// its steps, byte-identical.
///
/// **Rest step shape** (the harness contract):
/// ```json
/// { "action": "rest", "bonfire": 0, "anchor": "anchor/keeper-stand",
///   "pos": [44, 65, 2], "command": "/trigger dw.rest set 2" }
/// ```
/// The bot walks to `pos`, right-clicks the `dw_bonfire_<bonfire>` interaction —
/// which is what opens the dialog and what *enables* the trigger — and then sends
/// `command`, the exact chat line the "rest and save" button runs.
fn laid_out(
    plan: &Plan,
    layout: &plan::ExportedLayout,
    steps: Vec<Value>,
    witnesses: &[(String, Vec<Value>)],
) -> Vec<Value> {
    let mut steps: Vec<Option<Value>> = steps.into_iter().map(Some).collect();
    let mut out: Vec<Value> = Vec::with_capacity(layout.entries.len());
    for entry in &layout.entries {
        match entry {
            plan::Exported::Step(i) => out.push(steps[*i].take().expect("each step is laid once")),
            plan::Exported::Rest(cp) => {
                let bf = plan
                    .bonfires()
                    .find(|b| b.index == *cp)
                    .expect("a laid rest is a bonfire's");
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
            plan::Exported::Witness { assembly, k } => {
                let ws = witnesses
                    .iter()
                    .find(|(id, _)| id == assembly)
                    .map(|(_, ws)| ws)
                    .expect("a laid witness is an assembly's");
                out.push(ws[*k].clone());
            }
        }
    }
    out
}

pub(super) fn emit_critical_path(
    plan: &Plan,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    routes: &[crate::compiler::nav::LegRoute],
    layout: &plan::ExportedLayout,
    witnesses: &[(String, Vec<Value>)],
) -> Value {
    let en_route = crate::compiler::hold::en_route_holds(plan, &plan.critical_path, routes);
    // What one world shows after the exported path's ending (spec-0108 §6.3):
    // the lines the mainline's own story world fires — that world dates what
    // the mainline's replay cannot, an elective act's datum — and of those only
    // the ones decided there. A line the replay opened on an undatable term
    // fires in some worlds and not others, so no one world shows it.
    let flow = crate::compiler::flow::Flow::new(plan.campaign);
    let fired: BTreeSet<String> = match flow.mainline_story_world() {
        Some(k) => shown(&flow.story().runs[k].fired, &flow.story().runs[k].undecided),
        None => {
            let journal = flow.journal(&flow.playthrough());
            let undecided: BTreeSet<String> =
                journal.iter().flat_map(|j| j.undecided.clone()).collect();
            let fired: BTreeSet<String> = journal.into_iter().flat_map(|j| j.fired).collect();
            shown(&fired, &undecided)
        }
    };
    critical_path_json(
        plan,
        &plan.critical_path,
        &plan.critical_path_transport,
        &plan.critical_path_sneak,
        &plan.path_firing.keep_out,
        CutsceneHolds {
            after: &plan.critical_path_cutscene,
            en_route: &en_route,
        },
        moves,
        actor_moves,
        &fired,
        layout,
        witnesses,
    )
}

/// The lines a run fires that one world shows: every fired pointer whose gate
/// the run decided — `fired` without `undecided`.
pub(super) fn shown(fired: &BTreeSet<String>, undecided: &BTreeSet<String>) -> BTreeSet<String> {
    fired.difference(undecided).cloned().collect()
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

#[allow(clippy::too_many_arguments)]
pub(super) fn critical_path_json(
    plan: &Plan,
    walked: &[plan::Step],
    transports: &[Option<[i32; 3]>],
    sneak: &[bool],
    keep_out: &[Vec<crate::compiler::reach::KeepOut>],
    holds: CutsceneHolds<'_>,
    moves: &[crate::compiler::nav::MovePlan],
    actor_moves: &[crate::compiler::nav::ActorMovePlan],
    fired: &BTreeSet<String>,
    layout: &plan::ExportedLayout,
    witnesses: &[(String, Vec<Value>)],
) -> Value {
    // spec-0108 §6.3: what the world the ending left looks like, for the bot to
    // read after the campaign marker and the tail — each aftermath line of the
    // scene of every ending this path's replay fires, that fires itself.
    let aftermath = path_aftermath(plan, fired);
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
                // spec-0108 §6.5: an act the bot runs as the engine's own bundle
                // function, asserting the terms it changes before and after.
                Step::Mock { act, kind, function, before, after } => {
                    let term = |t: &plan::GateTerm| json!({
                        "objective": t.objective, "holder": t.holder(),
                        "min": t.min, "max": t.max, "negate": t.negate,
                    });
                    json!({
                        "action": "mock", "act": act, "kind": kind,
                        "function": format!("{}:{function}", plan.namespace),
                        "before": before.iter().map(term).collect::<Vec<_>>(),
                        "after": after.iter().map(term).collect::<Vec<_>>(),
                    })
                }
                Step::AssertComplete { objective, value } => {
                    let mut v = json!({
                        "action": "assert-complete", "scoreboard": { "objective": objective, "value": value }
                    });
                    if ending_tail > 0
                        && let Some(obj) = v.as_object_mut()
                    {
                        obj.insert("ending_tail_ticks".to_string(), json!(ending_tail));
                    }
                    if !aftermath.is_empty()
                        && let Some(obj) = v.as_object_mut()
                    {
                        obj.insert("aftermath".to_string(), Value::Array(aftermath.clone()));
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
            // The completion volumes this step keeps out of (`DW0311`): every
            // `reach` armed while the party performs it and not its own. The bot
            // keeps every walk of the step out of them by the selector's hitbox
            // rule. Present only when non-empty.
            if let Some(ko) = keep_out.get(i)
                && !ko.is_empty()
                && let Some(obj) = step.as_object_mut()
            {
                obj.insert(
                    "keep_out".to_string(),
                    Value::Array(
                        ko.iter()
                            .map(crate::compiler::reach::KeepOut::to_json)
                            .collect(),
                    ),
                );
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
    let steps = laid_out(plan, layout, steps, witnesses);
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
        "dw_codes": dw_codes(),
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
    // The test-level declaration (`world.test_level`, spec-0122 §7.2), carried
    // so the staging gate can read that this build was compiled from it. Absent
    // for every other campaign, which keeps their manifests byte-identical.
    if plan.campaign.world.content.test_level {
        manifest
            .as_object_mut()
            .expect("manifest is a JSON object")
            .insert("test_level".to_string(), Value::Bool(true));
    }
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

/// The aftermath a path's bot reads after the ending (spec-0108 §6.3): for each
/// ending whose `campaign-complete` the path's replay fires, every aftermath
/// effect of its scene that fires too, with what the bot can observe of it —
/// the §6.2 observable, or what a player's own body shows (`health` after
/// `damage-players`, an `effect` given or cleared) — or `emission-only`.
pub(super) fn path_aftermath(plan: &Plan, fired: &BTreeSet<String>) -> Vec<Value> {
    let scenes = crate::compiler::ending::Scenes::of(plan.campaign);
    let mut by_path: BTreeMap<String, &QuestEffect> = BTreeMap::new();
    delvewright_dsl::for_each_campaign_effect(plan.campaign, &mut |p, _, e| {
        by_path.insert(p.to_string(), e);
    });
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let mut out = Vec::new();
    for e in &scenes.endings {
        if !fired.contains(&e.path) || !seen.insert(e.scene) {
            continue;
        }
        for x in &scenes.scenes[e.scene].aftermath {
            if !fired.contains(&x.path) {
                continue;
            }
            let Some(eff) = by_path.get(&x.path) else {
                continue;
            };
            let observable = match &eff.verb {
                Verb::DamagePlayers { amount, .. } => json!({"kind": "health", "amount": amount}),
                Verb::GiveEffect { effect, .. } => json!({"kind": "effect", "effect": effect}),
                Verb::ClearEffect { effect, .. } => {
                    json!({"kind": "no-effect", "effect": effect})
                }
                _ => observation(plan, eff).unwrap_or(Value::String("emission-only".into())),
            };
            out.push(json!({"path": x.path, "verb": x.verb, "observable": observable}));
        }
    }
    out
}
