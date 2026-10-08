//! Dialogue: the variant cap, gated options, and the dialogs.

use super::*;

/// The most gated options one dialogue node may declare.
///
/// Vanilla has no conditional option inside a `dialog`, so the compiler encodes
/// visibility by **precomputing every combination**: `n` gated options emit `2^n`
/// dialog JSONs plus a `2^n`-clause dispatcher keyed on a `dw.dmask` bitmask. Ten
/// is 1024 variants for a single node — already an order of magnitude past
/// anything authorable (the largest node in any shipped campaign gates four), and
/// the point past which the pack size, not the author, decides what the delve is.
///
/// There is a hard wall behind the soft one: the mask is built with `1u32 << i`
/// (undefined past 31, a debug-build panic at 32 — the original symptom of this
/// gap) and compared against a Minecraft scoreboard, i.e. an `i32`, so bit 31 is
/// unrepresentable at runtime regardless. The cap keeps the build well clear of
/// both, and turns a compiler panic into a coded content diagnostic that names the
/// node.
pub const MAX_GATED_DIALOGUE_OPTIONS: usize = 10;

/// Fail the build if any dialogue node exceeds [`MAX_GATED_DIALOGUE_OPTIONS`]
/// (`DW0362`). Runs before any variant emission so the `1u32 << n` shifts in
/// `gated_node_choosers` / `emit_dialogs` are unreachable past the cap.
pub(super) fn check_dialogue_variant_cap(plan: &Plan) -> Result<(), BuildFailure> {
    for npc in &plan.npcs {
        let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for opt in &npc.options {
            if !seen.insert(opt.node_id.as_str()) {
                continue;
            }
            let n = node_gated_options(npc, &opt.node_id).len();
            if n > MAX_GATED_DIALOGUE_OPTIONS {
                return Err(BuildFailure::Diagnostic {
                    code: DW_DIALOGUE_VARIANT_CAP,
                    message: format!(
                        "dialogue node `{}` on npc `{}` declares {n} conditionally-visible \
                         options (`requires_flags` / `forbids_flags` / a `complete-objective` \
                         effect); the cap is {MAX_GATED_DIALOGUE_OPTIONS}. Vanilla cannot hide a \
                         dialog option, so the compiler precomputes every combination: this node \
                         would emit 2^{n} dialog variants and a dispatcher of the same size. \
                         Split the node into a short chain of nodes, or move some of the gating \
                         onto the objective that reaches the node.",
                        opt.node_id, npc.npc_id,
                    ),
                });
            }
        }
    }
    Ok(())
}

/// Whether an option's display is gated (DSL v0.4+): it requires flags (flag
/// axis), forbids flags (v0.6 negative flag axis), or completes an objective
/// (objective-state axis — visible only while that objective is active). Below
/// v0.4 nothing is display-gated, so v0.2/v0.3 nodes stay byte-identical.
/// `requires_flags` is itself a v0.4 verb (`forbids_flags` v0.6), so the whole
/// predicate collapses to `false` pre-v0.4.
pub(super) fn option_display_gated(opt: &plan::OptionPlan) -> bool {
    !opt.requires_flags.is_empty()
        || !opt.forbids_flags.is_empty()
        // v0.10 (spec-0031): a numeric gate hides the option exactly as a flag
        // gate does — an option the player cannot pick must not be drawn.
        || !opt.requires_state.is_empty()
        || !opt.completes.is_empty()
}

/// The display-gated options of `node_id`, in declared order — the bit order of
/// the node's per-player availability mask (`dw.dmask`). Empty for an ungated
/// node (v0.2/v0.3, or a v0.4 node whose every option is unconditional).
pub(super) fn node_gated_options<'a>(
    npc: &'a plan::NpcPlan,
    node_id: &str,
) -> Vec<&'a plan::OptionPlan> {
    npc.options
        .iter()
        .filter(|o| o.node_id == node_id && option_display_gated(o))
        .collect()
}

/// The ` if …`/` unless …` execute fragment (leading space) that is satisfied
/// exactly when `opt` should be DISPLAYED: every `requires_flags` flag set (flag
/// axis), and — v0.4+ — every completed objective's quest active and the
/// objective itself not yet complete (objective-state axis). Mirrors the
/// click-handler guard (emit.rs ~1166) so an option is shown iff clicking it
/// would fire.
pub(super) fn option_display_conditions(
    plan: &Plan,
    c: &delvewright_dsl::Campaign,
    opt: &plan::OptionPlan,
) -> String {
    let p = plan::PARTY;
    let mut cond = String::new();
    for f in &opt.requires_flags {
        cond.push_str(&format!(" if score {p} {} matches 1", plan::flag_score(f)));
    }
    // v0.6 negative gate: hidden once any forbidden flag is set (`unless …
    // matches 1` treats an unset score as "not set").
    for f in &opt.forbids_flags {
        cond.push_str(&format!(
            " unless score {p} {} matches 1",
            plan::flag_score(f)
        ));
    }
    // DSL v0.10 (spec-0031). A dialogue option's availability is computed PER
    // PLAYER (`dw.dmask`, run `as @s`), so this is the one gate site a
    // `player`-scoped datum reads from `@s` rather than from the party holder.
    cond.push_str(&state_cond(plan, &opt.requires_state, false));
    // The objective-state axis is the objective's WHOLE pending guard
    // (spec-0093 §6.3): quest active ∧ every `after` complete ∧ its gate ∧ not
    // yet complete — the same guard every other objective driver goes through.
    // A button drawn before its beat is pending is the island's muster/surf
    // softlock; it is not drawn.
    for obj in &opt.completes {
        if let Some((qid, o)) = objective_quest(c, obj) {
            cond.push_str(&pending_guard(plan, o, &quest_active_score(qid)));
        }
    }
    cond
}

/// The command that displays `node_id`: a direct `dialog show` for an ungated
/// node, or the availability chooser function for a gated one (which shows the
/// variant matching the player's satisfied flags + active objectives).
pub(super) fn show_node_cmd(plan: &Plan, npc: &plan::NpcPlan, node_id: &str) -> String {
    let ns = &plan.namespace;
    let node_safe = plan::safe_local(node_id);
    if node_gated_options(npc, node_id).is_empty() {
        format!("dialog show @s {ns}:{}_{}", npc.safe, node_safe)
    } else {
        format!("function {ns}:show_{}_{}", npc.safe, node_safe)
    }
}

/// Availability chooser + mask functions for this NPC's display-gated nodes. Per
/// gated node, two functions:
///
/// * `dmask_<npc>_<node>` computes the per-player availability bitmask into
///   `dw.dmask` — bit `i` set iff the node's `i`-th gated option is currently
///   displayable (flags satisfied and every completed objective active + not yet
///   complete). Pure scoreboard math, so a PackTest can drive it and assert the
///   mask without opening a dialog.
/// * `show_<npc>_<node>` runs the mask function, then `dialog show`s the variant
///   (`<npc>_<node>__m<mask>`) whose visible options match.
pub(super) fn gated_node_choosers(plan: &Plan, npc: &plan::NpcPlan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut out = Vec::new();
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for opt in &npc.options {
        if !seen.insert(opt.node_id.as_str()) {
            continue;
        }
        let gated = node_gated_options(npc, &opt.node_id);
        if gated.is_empty() {
            continue;
        }
        let node_safe = plan::safe_local(&opt.node_id);

        let mut dmask = vec!["scoreboard players set @s dw.dmask 0".to_string()];
        for (i, g) in gated.iter().enumerate() {
            dmask.push(format!(
                "execute{} run scoreboard players add @s dw.dmask {}",
                option_display_conditions(plan, c, g),
                1u32 << i
            ));
        }
        out.push((format!("dmask_{}_{}", npc.safe, node_safe), lines(&dmask)));

        let mut show = vec![format!("function {ns}:dmask_{}_{}", npc.safe, node_safe)];
        for mask in 0..(1u32 << gated.len()) {
            show.push(format!(
                "execute if score @s dw.dmask matches {mask} run dialog show @s {ns}:{}_{}__m{mask}",
                npc.safe, node_safe
            ));
        }
        out.push((format!("show_{}_{}", npc.safe, node_safe), lines(&show)));
    }
    out
}

/// Whether any dialogue option is display-gated (gates the `dw.dmask`
/// declaration): a v0.4+ option that requires flags or completes an objective.
pub(super) fn has_gated_dialogue(c: &delvewright_dsl::Campaign) -> bool {
    use delvewright_dsl::DialogueEffect;
    c.dialogue
        .content
        .dialogues
        .iter()
        .flat_map(|t| &t.nodes)
        .flat_map(|n| &n.options)
        .any(|o| {
            !o.requires_flags.is_empty()
                || !o.forbids_flags.is_empty()
                || o.effects
                    .iter()
                    .any(|e| matches!(e, DialogueEffect::CompleteObjective { .. }))
        })
}

/// **One dialog button** (spec-0078): every button the engine presents to a
/// player — class selection, a bonfire's two options, a shop offer, a dialogue
/// option — is built here and nowhere else, so every one of them carries the
/// same optional hover `tooltip`.
///
/// Vanilla's 1.21.11 action button is `CommonButtonData` (`label`, optional
/// `tooltip`, `width`) plus an optional action; see [`build_node_dialog`] for the
/// codec proof. The label arrives already built (a bonfire's is chrome-rebound);
/// a stated tooltip becomes `tr(tooltip)` and an absent one emits no key, so a
/// campaign that states none is byte-identical. The action is a `/trigger`,
/// the only command a non-op player may run.
pub(super) fn dialog_button(label: Value, tooltip: Option<&str>, command: &str) -> Value {
    let mut button = serde_json::Map::new();
    button.insert("label".to_string(), label);
    if let Some(t) = tooltip {
        button.insert("tooltip".to_string(), tr(t));
    }
    button.insert(
        "action".to_string(),
        json!({ "type": "minecraft:run_command", "command": command }),
    );
    Value::Object(button)
}

pub(super) fn emit_dialogs(plan: &Plan, chrome: &delvewright_dsl::Chrome) -> Vec<(String, Value)> {
    let c = plan.campaign;
    let mut dialogs = Vec::new();

    // class selection
    let actions: Vec<Value> = plan
        .classes
        .iter()
        .zip(&c.classes.content.classes)
        .map(|(cp, class)| {
            // spec-0078: the class button's tooltip is its required `blurb`.
            dialog_button(
                tr(&class.name),
                Some(&class.blurb),
                &format!("/trigger dw.class set {}", cp.n),
            )
        })
        .collect();
    dialogs.push((
        "class_select".to_string(),
        json!({
            "type": "minecraft:multi_action",
            "title": tr(&chrome.get(delvewright_dsl::chrome::CLASS_TITLE)),
            "body": [{ "type": "minecraft:plain_message",
                       "contents": tr(&chrome.get(delvewright_dsl::chrome::CLASS_BODY)) }],
            "columns": 1,
            "can_close_with_escape": false,
            "after_action": "close",
            "actions": actions
        }),
    ));

    // spec-0016 §1: one bonfire dialog per bonfire,
    // offering EXACTLY two options — rest and save, or save only. Nothing else
    // may appear here: the ruling is that a campfire is a real interaction with a
    // real choice, not a one-click "arrive" objective. Emitted only for a campaign
    // with a bonfire → byte-identical otherwise.
    for bf in plan.bonfires() {
        let i = bf.index;
        dialogs.push((
            format!("bonfire_{i}"),
            json!({
                "type": "minecraft:multi_action",
                "title": tr(&chrome.rebind(&bf.prompt)),
                "columns": 1,
                "can_close_with_escape": true,
                "after_action": "close",
                "actions": [
                    dialog_button(
                        tr(&chrome.rebind(&bf.rest_label)),
                        bf.rest_tooltip.as_deref(),
                        "/trigger dw.rest set 2",
                    ),
                    dialog_button(
                        tr(&chrome.rebind(&bf.save_label)),
                        bf.save_tooltip.as_deref(),
                        "/trigger dw.rest set 1",
                    )
                ]
            }),
        ));
    }

    // spec-0032: one dialog per shop. The offers are the buttons, in declaration
    // order, and each runs `/trigger dw.shop set <n>` — the same channel the
    // bonfire's two options use, because `/trigger` is the only command a non-op
    // player may run. `DW0523` guarantees the action list is non-empty: vanilla's
    // 1.21.11 dialog codec rejects an empty one at pack load.
    for (i, sh, _) in shops(plan) {
        let actions: Vec<Value> = sh
            .offers
            .iter()
            .enumerate()
            .map(|(j, off)| {
                dialog_button(
                    tr(&off.label),
                    off.tooltip.as_deref(),
                    &format!("/trigger dw.shop set {}", j + 1),
                )
            })
            .collect();
        dialogs.push((
            format!("shop_{i}"),
            json!({
                "type": "minecraft:multi_action",
                "title": tr(&sh.title),
                "columns": 1,
                "can_close_with_escape": true,
                "after_action": "close",
                "actions": actions
            }),
        ));
    }

    // per-npc dialogue nodes (stage 6) → one dialog each
    for npc in &plan.npcs {
        let dsl_npc = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|n| n.id.as_str() == npc.npc_id);
        let Some(dsl_npc) = dsl_npc else { continue };
        let Some(tree) = c.dialogue.content.tree_for(&npc.npc_id) else {
            continue;
        };
        for node in &tree.nodes {
            let node_opts: Vec<&plan::OptionPlan> = npc
                .options
                .iter()
                .filter(|o| o.node_id == node.id.as_str())
                .collect();
            let node_safe = plan::safe_local(node.id.as_str());
            let gated = node_gated_options(npc, node.id.as_str());
            if gated.is_empty() {
                // Ungated node → a single dialog (byte-identical to v0.2/v0.3, or a
                // v0.4 node whose every option is unconditional).
                dialogs.push((
                    format!("{}_{node_safe}", npc.safe),
                    build_node_dialog(
                        &dsl_npc.name,
                        &node.text,
                        &node_opts,
                        &npc.trigger_objective,
                    ),
                ));
            } else {
                // v0.4 display-gated node → one variant per availability bitmask.
                // Bit `i` (declared order among gated options) means "the i-th gated
                // option is displayable now": every flag it needs is set (flag axis)
                // and every objective it completes is active (objective-state axis).
                // The chooser function (`show_<npc>_<node>`) computes the live mask
                // and shows the matching variant, so a gated option is genuinely
                // absent until it is displayable (spec-0008 §1).
                for mask in 0..(1u32 << gated.len()) {
                    let mut gi = 0u32;
                    let visible: Vec<&plan::OptionPlan> = node_opts
                        .iter()
                        .copied()
                        .filter(|o| {
                            if option_display_gated(o) {
                                let bit = gi;
                                gi += 1;
                                mask & (1u32 << bit) != 0
                            } else {
                                true
                            }
                        })
                        .collect();
                    dialogs.push((
                        format!("{}_{node_safe}__m{mask}", npc.safe),
                        build_node_dialog(
                            &dsl_npc.name,
                            &node.text,
                            &visible,
                            &npc.trigger_objective,
                        ),
                    ));
                }
            }
        }
    }
    dialogs
}

/// Build one node dialog from its (already flag-filtered) options. A node with no
/// visible options is a terminal `minecraft:notice` (an empty `multi_action`
/// action list crashes the 1.21.11 dialog codec at load — gap 10); otherwise a
/// `minecraft:multi_action` whose buttons fire each option's `/trigger`.
///
/// **The button's `tooltip` (v0.8).** Vanilla's dialog action button is
/// `ActionButton(CommonButtonData, Optional<DialogAction>)`, and
/// `CommonButtonData`'s codec is exactly `label` (a text component) +
/// `tooltip` (an *optional* text component) + `width` (default 150) — verified
/// against the pinned 1.21.11 client jar's codec, not folklore. The client's
/// `DialogControlSet` turns a present `tooltip` into `Tooltip.create(component)`
/// and hangs it on the button, so it renders as an ordinary hover box (wrapped at
/// 170 px), never on the button face. That is why `DW0331` does not reach it: a
/// tooltip wraps, it does not scroll. An option with no `tooltip` emits no key —
/// a pre-0.8 campaign's dialogs are byte-identical.
pub(super) fn build_node_dialog(
    npc_name: &str,
    text: &str,
    opts: &[&plan::OptionPlan],
    trigger_objective: &str,
) -> Value {
    if opts.is_empty() {
        json!({
            "type": "minecraft:notice",
            "title": tr(npc_name),
            "body": [{ "type": "minecraft:plain_message", "contents": tr(text) }],
            "can_close_with_escape": true
        })
    } else {
        let actions: Vec<Value> = opts
            .iter()
            .map(|o| {
                dialog_button(
                    tr(&o.label),
                    o.tooltip.as_deref(),
                    &format!("/trigger {trigger_objective} set {}", o.n),
                )
            })
            .collect();
        json!({
            "type": "minecraft:multi_action",
            "title": tr(npc_name),
            "body": [{ "type": "minecraft:plain_message", "contents": tr(text) }],
            "columns": 1,
            "can_close_with_escape": true,
            "after_action": "close",
            "actions": actions
        })
    }
}

/// Every NPC's dialog option handlers, its talk dispatch, its cast selector and barks, and its gated-node choosers.
pub(super) fn dialog_handler_fns(
    plan: &Plan,
    casts: &std::collections::BTreeMap<String, crate::compiler::cast::NpcCast>,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();
    for npc in &plan.npcs {
        for opt in &npc.options {
            let mut body: Vec<String> = Vec::new();
            body.push(format!(
                "scoreboard players reset @s {}",
                npc.trigger_objective
            ));
            // Re-arm the trigger IN THIS FUNCTION, immediately after consuming it.
            //
            // `reset` both clears the score and re-locks the trigger, and the only
            // other re-enable is the per-tick `scoreboard players enable @a` at the
            // top of `tick`. On a dedicated server that is invisible: the handler
            // runs inside tick N, the next tick re-enables, and the player's next
            // click lands in tick N+1 or later. On the **integrated (singleplayer)
            // server** it is a real hole — 1.21.9+ freezes the integrated server
            // while a screen is open, and the last thing this handler does is show
            // the next dialog node. So: tick N re-enables, dispatches here, we lock
            // the trigger, we open the next screen, ticking STOPS. The player's
            // click is queued and executed the instant ticking resumes — before the
            // tick function's re-enable — and vanilla rejects it ("You can't
            // trigger this objective yet"), silently swallowing one dialogue
            // choice. A dedicated server never pauses, so no rung of the validation
            // ladder can reproduce it.
            //
            // Placed here rather than at the end of the body on purpose: the
            // flag-gate below can `return fail`, and an end-of-body re-enable would
            // be skipped on exactly the path that consumed the trigger without
            // doing anything. Nothing below re-locks it, so this position strictly
            // dominates. `enable` on an unset score initialises it to 0, which
            // matches no dispatch guard (option values are 1-based).
            //
            // The per-tick `enable @a` stays as belt-and-braces.
            body.push(format!(
                "scoreboard players enable @s {}",
                npc.trigger_objective
            ));
            // v0.4: a flag-gated option is inert until its flags are set — so a
            // direct `/trigger` (the bot's path, which bypasses the UI variant
            // hiding) cannot fire it early. `return fail` short-circuits the rest.
            // The story flags are party state (spec-0018): what one player learned
            // from an NPC opens the option for whoever next speaks to them.
            for f in &opt.requires_flags {
                body.push(format!(
                    "execute unless score {} {} matches 1 run return fail",
                    plan::PARTY,
                    plan::flag_score(f)
                ));
            }
            // v0.6: the negative gate — a `forbids_flags`-suppressed option is
            // equally inert to a direct `/trigger` once any listed flag is set.
            for f in &opt.forbids_flags {
                body.push(format!(
                    "execute if score {} {} matches 1 run return fail",
                    plan::PARTY,
                    plan::flag_score(f)
                ));
            }
            // v0.10: the numeric gate, made inert to a direct `/trigger` the same
            // way. One `return fail` per term — any single comparison failing
            // shuts the option — which is what `negate` spells.
            for clause in state_clauses(plan, &opt.requires_state, true) {
                body.push(format!("execute {clause} run return fail"));
            }
            // v0.4: set any flags this option declares (dialogue `set-flag`).
            for f in &opt.sets_flags {
                body.push(format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    plan::flag_score(f)
                ));
            }
            // v0.5: world time / weather cuts this option declares (dialogue
            // `set-time`/`set-weather`, spec-0010). Dimension-global instant cuts.
            for t in &opt.sets_time {
                let world = c.world.content.time;
                body.push(format!(
                    "time set {}",
                    t.token(t.clock(delvewright_dsl::TimeSite::Cut, world))
                ));
            }
            for w in &opt.sets_weather {
                body.push(format!("weather {}", w.token()));
            }
            // v0.6: party-wide respawn checkpoints this option sets (dialogue
            // `set-checkpoint`, spec-0012).
            for (anchor, on_respawn) in &opt.sets_checkpoints {
                emit_set_checkpoint(plan, anchor, on_respawn, &mut body);
            }
            // v0.6: deferred NPCs this option brings into the world (dialogue
            // `spawn-npc`) — a character walking in mid-conversation.
            for n in &opt.spawns_npcs {
                body.push(format!("function {ns}:{}", spawn_npc_fn(n)));
            }
            // The click completes the objective under the same pending guard the
            // button is drawn under (spec-0093 §6.3): a press that arrives while
            // the objective is not pending completes nothing.
            for obj in &opt.completes {
                if let Some((qid, o)) = objective_quest(c, obj) {
                    body.push(format!(
                        "execute{} run function {ns}:complete_{}",
                        pending_guard(plan, o, &quest_active_score(qid)),
                        safe_obj_fn(obj),
                    ));
                }
            }
            if let Some(next) = &opt.next {
                body.push(show_node_cmd(plan, npc, next));
            }
            fns.push((format!("dlg_{}_{}", npc.safe, opt.n), lines(&body)));
        }
        // keeper interaction reward: consume the interaction record, then show
        // whatever the cast ledger says this NPC's right-click offers right now
        // (spec-0020). With no ledger this is the single root line it always was.
        let mut talk = vec![format!(
            "advancement revoke @s only {ns}:{}_interact",
            npc.safe
        )];
        talk.extend(cast_dispatch(plan, npc, casts));
        fns.push((format!("talk_{}", npc.safe), lines(&talk)));
        fns.extend(cast_selector_fn(plan, npc, casts));
        fns.extend(cast_bark_fns(plan, npc, casts));
        // v0.4: flag-gate chooser functions for gated nodes.
        for func in gated_node_choosers(plan, npc) {
            fns.push(func);
        }
    }
    fns
}
