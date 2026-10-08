//! Quests: arming order, pending guards, the lobby and the outro.

use super::*;

/// The closing line on the completion advancement: the authored `world.outro`
/// (l10n key `world.outro`), else the finale quest's `goal` (key
/// `quest.<id>.goal`) — the thing the party just accomplished, already inventoried
/// and translated. Both are campaign content, so no English is baked in here.
/// Falls back to the delve title only if the finale names no planned quest, which
/// cross-stage validation (`DW0160`) already rejects.
pub(super) fn campaign_outro(c: &delvewright_dsl::Campaign) -> String {
    if let Some(outro) = &c.world.content.outro {
        return outro.clone();
    }
    let finale = c.quest_plan.content.finale.as_str();
    c.quest_plan
        .content
        .quests
        .iter()
        .find(|q| q.id.as_str() == finale)
        .map(|q| q.goal.clone())
        .unwrap_or_else(|| c.world.content.title.clone())
}

/// The `dw.sys` fake player holding the live online-player count, recomputed each
/// tick — the lobby gate's only input (spec-0018 `world.min_players`). Emitted
/// only for a campaign that declares `min_players >= 2`.
pub(super) const LOBBY_COUNT: &str = "#lobby";

/// The lobby's waiting message (spec-0018): a live "x / n" actionbar for players
/// who have not taken a class yet, while the party is short. The count is a
/// vanilla `score` component reading [`LOBBY_COUNT`], so it updates itself
/// without any per-count emission. English-first (CLAUDE.md language policy); a
/// compiler default, not an authored string, so it is not l10n-inventoried.
pub(super) fn lobby_actionbar(min_players: u8, chrome: &delvewright_dsl::Chrome) -> String {
    // One sentence with two `with` arguments — the live count and the size the
    // delve requires — so a translation orders them for its own language instead
    // of inheriting English's `<prefix> <n> / <n>`.
    tr_with(
        &chrome.get(delvewright_dsl::chrome::LOBBY_WAITING),
        &[
            ("color", json!("yellow")),
            (
                "with",
                json!([
                    { "score": { "name": LOBBY_COUNT, "objective": "dw.sys" }, "color": "gold" },
                    { "text": min_players.to_string(), "color": "gold" }
                ]),
            ),
        ],
    )
    .to_string()
}

/// The intra-quest activation + pending guard for an objective (v0.3): the quest
/// must be active, every `after` prerequisite and `requires_flags` flag set, no
/// `forbids_flags` flag set (v0.6 negative gate; `unless … matches 1` so an
/// unset score counts as "not set"), and the objective itself not yet complete.
/// Returns the ` if …`/` unless …` fragment (leading space); callers append the
/// type-specific condition + `run`, prepending `execute as @a` when they need a
/// player to test (proximity, inventory, a fired trigger) and a bare `execute`
/// otherwise.
///
/// **Every term reads the party holder** (spec-0018). That is what makes an
/// `after: [obj/a, obj/b]` AND-join a division of labor: player A clearing
/// `obj/a` in one room and player B clearing `obj/b` in another both write
/// `#party`, so the successor's guard opens for the whole party. It is also what
/// keeps the drivers single-fire under `as @a`: vanilla evaluates the conditions
/// per selected player in turn, so the first player's `run` sets the party score
/// and every later player's `unless score #party …` fails in the same tick.
/// Stage-5 quests in **arming order**: a quest whose completion arms another is
/// visited before the quest it arms.
///
/// The arming graph is exactly the `Trigger::QuestComplete` edges — the only two
/// trigger kinds are `CampaignStart` (a root, armed by `setup`) and
/// `QuestComplete`, so this is the whole of it.
///
/// **Stable.** Declaration order breaks every tie and seeds the ready set, so a
/// campaign already declared in arming order — which every campaign built so far
/// is — comes back in exactly its declared order and emits byte-identically. It
/// is also **total**: a cycle (already an error elsewhere; a quest cannot arm
/// itself through any path and still be reachable) leaves an unresolved tail,
/// which is appended in declaration order rather than dropped, because a lost
/// quest would be a far worse failure than a badly-ordered one.
pub(super) fn quests_in_arming_order(
    c: &delvewright_dsl::Campaign,
) -> Vec<&delvewright_dsl::Quest> {
    let quests = &c.quests.content.quests;
    let index: BTreeMap<&str, usize> = quests
        .iter()
        .enumerate()
        .map(|(i, q)| (q.id.as_str(), i))
        .collect();
    let mut indegree = vec![0usize; quests.len()];
    let mut arms: Vec<Vec<usize>> = vec![Vec::new(); quests.len()];
    for (i, q) in quests.iter().enumerate() {
        if let Trigger::QuestComplete { quest } = &q.trigger
            && let Some(&p) = index.get(quest.as_str())
            && p != i
        {
            indegree[i] += 1;
            arms[p].push(i);
        }
    }
    // Kahn's algorithm with a declaration-ordered ready queue: among quests that
    // become ready together, the earliest-declared is always emitted first.
    let mut ready: Vec<usize> = (0..quests.len()).filter(|&i| indegree[i] == 0).collect();
    let mut order: Vec<usize> = Vec::with_capacity(quests.len());
    while !ready.is_empty() {
        let i = ready.remove(0);
        order.push(i);
        for &dep in &arms[i] {
            indegree[dep] -= 1;
            if indegree[dep] == 0 {
                let pos = ready.partition_point(|&r| r < dep);
                ready.insert(pos, dep);
            }
        }
    }
    let mut seen = vec![false; quests.len()];
    let mut out: Vec<&delvewright_dsl::Quest> = Vec::with_capacity(quests.len());
    for i in order {
        seen[i] = true;
        out.push(&quests[i]);
    }
    for (i, q) in quests.iter().enumerate() {
        if !seen[i] {
            out.push(q);
        }
    }
    out
}

pub(super) fn pending_guard(plan: &Plan, o: &Objective, quest_active: &str) -> String {
    let p = plan::PARTY;
    let mut g = format!(" if score {p} {quest_active} matches 1");
    for a in o.after() {
        g.push_str(&format!(
            " if score {p} {} matches 1",
            obj_score(a.as_str())
        ));
    }
    // The objective's whole gate, in gate field order: required flags, forbidden
    // flags, then (DSL v0.10) the numeric terms. Written through `gate_cond` so
    // this guard cannot end up knowing about two of the gate's three fields —
    // which is exactly how the numeric axis would have been missed. Empty terms
    // contribute nothing, so a pre-0.10 campaign's guard is byte-identical.
    g.push_str(&gate_cond(plan, o.gate()));
    g.push_str(&format!(
        " unless score {p} {} matches 1",
        obj_score(o.id().as_str())
    ));
    g
}

/// Every objective's activation and completion functions, and each quest's `complete_q_<q>`.
pub(super) fn quest_completion_fns(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    branch_transport: &BranchTransportOverlay,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();
    for q in &c.quests.content.quests {
        let q_area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            let oid = o.id().as_str();
            // v0.3 activation function (gap 13): run once when the objective
            // activates (driven from `tick`) — set the global once-flag, then place
            // the objective's prop(s). Emitted only for objectives with a prop.
            let cmds = activation_commands(plan, q_area, o);
            if !cmds.is_empty() {
                let mut act = vec![format!(
                    "scoreboard players set {} dw.sys 1",
                    activation_flag(oid)
                )];
                act.extend(cmds);
                fns.push((format!("activate_{}", safe_obj_fn(oid)), lines(&act)));
            }
            // v0.3 objective-activation feedback (M2 fix 4): the announce function
            // shows the title + hint once and plays a subtle sound. Emitted only
            // for ANNOUNCED objectives (spec-0093): a title whose resolved
            // `announcement` is `shown`. Nothing for v0.2, nothing for a quiet one.
            if o.announced(&c.quests.content.guidance)
                && let Some(title) = o.title()
            {
                // spec-0018: the objective is the PARTY's, so its title, hint and
                // cue address `@a` and the once-latch lives on the party holder —
                // one announcement per objective, heard by everyone, never a
                // per-player replay for whoever happened to be standing nearby.
                let mut ann: Vec<String> = Vec::new();
                ann.push(format!(
                    "tellraw @a {}",
                    // One sentence, one key: the title is a `with` argument rather
                    // than a second component, so a translation decides where the
                    // title sits (chrome::OBJECTIVE_NEW). `bold: false` on the
                    // argument keeps the title unbolded now that it inherits the
                    // prefix's style instead of standing beside it.
                    tr_with(
                        &chrome.get(delvewright_dsl::chrome::OBJECTIVE_NEW),
                        &[
                            ("color", json!("yellow")),
                            ("bold", json!(true)),
                            (
                                "with",
                                json!([tr_with(
                                    title,
                                    &[("color", json!("gold")), ("bold", json!(false))]
                                )])
                            ),
                        ],
                    )
                ));
                if let Some(hint) = o.hint() {
                    ann.push(format!(
                        "tellraw @a {}",
                        tr_with(hint, &[("color", json!("gray")), ("italic", json!(true))])
                    ));
                }
                ann.push("playsound minecraft:block.note_block.pling player @a".to_string());
                ann.push(format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    announce_score(oid)
                ));
                fns.push((format!("announce_{}", safe_obj_fn(oid)), lines(&ann)));
            }

            let mut body: Vec<String> = Vec::new();
            // The completing action advances the PARTY (spec-0018) — this single
            // write is what lets two players clear two arms of an AND-join in two
            // rooms and unlock the successor for both.
            body.push(format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                obj_score(oid)
            ));
            // Machine completion-marker for the validation bot, broadcast the
            // instant this objective's score flips — BEFORE any effect that may
            // teleport, open a cutscene or complete the campaign, so the harness
            // observes each objective's own completion in path order. The critical
            // path names the objective a step must prove; this is the only evidence
            // the bot accepts for it (see `plan::marker_line`). Player chat can
            // never start with the sigil and `DW0182` reserves it in authored /
            // translated text, so it cannot be forged. `@a` for the same reason the
            // campaign marker uses it: a bot filling a seat in a multiplayer delve
            // must still see it.
            body.push(format!(
                "tellraw @a {}",
                json!({
                    "text": plan::marker_line(ns, oid),
                    "color": "dark_gray"
                })
            ));
            // v0.3 objective-completion feedback (M2 fix 4): a confirmation line +
            // sound so progress is legible. Announced objectives only (spec-0093);
            // v0.2 unchanged.
            if o.announced(&c.quests.content.guidance)
                && let Some(title) = o.title()
            {
                body.push(format!(
                    "tellraw @a {}",
                    tr_with(
                        &chrome.get(delvewright_dsl::chrome::OBJECTIVE_COMPLETE),
                        &[
                            ("color", json!("green")),
                            (
                                "with",
                                json!([tr_with(title, &[("color", json!("white"))])])
                            ),
                        ],
                    )
                ));
                body.push("playsound minecraft:entity.experience_orb.pickup player @a".to_string());
            }
            // Objective-marker lifecycle: despawn every ENTITY this
            // objective's activation summoned, so a completed interact/reach
            // objective leaves nothing behind. Two motivations, strongest first:
            // (1) a finished interact objective must not remain clickable — its
            // `minecraft:interaction` hitbox is a game-design correctness issue, not
            // mere clutter; (2) the leaked hitboxes and wayfinding item_displays are
            // non-colliding but congest the critical-path bot's pathfinding around
            // later NPCs. Prop BLOCKS (spec-0008 interact prop, collect chest) are
            // the affordance itself — real world blocks, intended scenery — so they
            // persist; only summoned entities are removed. Gated identically to the
            // summon (a non-empty activation) so objectives with no summon emit
            // nothing here.
            if !activation_commands(plan, q_area, o).is_empty() {
                body.extend(completion_cleanup(o));
            }
            // `complete_<obj>` is dispatched `as @a` from `tick`, so this bundle
            // runs with the acting player as `@s` (see `Audience::Party`).
            body.extend(emit_effect_bundle(
                plan,
                objective_effects(c, oid),
                root_audience(delvewright_dsl::EffectRootKind::ObjectiveComplete),
            ));
            // Inter-area transport: if completing this objective moves the player
            // into a different area on the critical path, teleport them to that
            // area's entry spawn (areas are AREA_SPACING apart across void). Runs
            // after gate effects so the destination area is already unlocked.
            if let Some(pos) = plan.transport.get(oid) {
                body.push(format!("teleport @s {} {} {}", pos[0], pos[1], pos[2]));
            }
            // A crossing that happens only on ONE branch. The exported
            // path never walks it, so it cannot be unconditional — it is gated on
            // exactly the flag assignment that selects its branch, the same
            // `#party` predicate a branch-gated dialogue option uses. Prefix
            // conditions do not rebind `@s`, so the acting player is still the
            // one carried. Empty for every campaign whose branches cross only
            // where the exported path already does (byte-identity).
            let mut emitted: BTreeSet<String> = BTreeSet::new();
            for row in branch_transport.get(oid).into_iter().flatten() {
                let tp = format!("teleport @s {} {} {}", row.pos[0], row.pos[1], row.pos[2]);
                let mut cmd = String::new();
                for f in &row.set {
                    cmd.push_str(&format!(
                        " if score {} {} matches 1",
                        plan::PARTY,
                        plan::flag_score(f)
                    ));
                }
                for f in &row.unset {
                    cmd.push_str(&format!(
                        " unless score {} {} matches 1",
                        plan::PARTY,
                        plan::flag_score(f)
                    ));
                }
                // A branch that pins no flags at all is indistinguishable at
                // runtime: there is nothing to condition on, so the crossing is
                // simply unconditional.
                let line = if cmd.is_empty() {
                    tp
                } else {
                    format!("execute{cmd} run {tp}")
                };
                if emitted.insert(line.clone()) {
                    body.push(line);
                }
            }
            body.push(format!(
                "function {ns}:check_q_{}",
                plan::safe_local(q.id.as_str())
            ));
            fns.push((format!("complete_{}", safe_obj_fn(oid)), lines(&body)));
        }

        // check_q_<quest>
        // The quest-level AND (every objective done) is a party predicate too:
        // whoever finishes the LAST objective completes the quest for everyone.
        let mut check: Vec<String> = Vec::new();
        let mut guard = "execute".to_string();
        for o in &q.objectives {
            guard.push_str(&format!(
                " if score {} {} matches 1",
                plan::PARTY,
                obj_score(o.id().as_str())
            ));
        }
        guard.push_str(&format!(
            " unless score {} {} matches 1 run function {ns}:complete_q_{}",
            plan::PARTY,
            quest_score(q.id.as_str()),
            plan::safe_local(q.id.as_str())
        ));
        check.push(guard);
        fns.push((
            format!("check_q_{}", plan::safe_local(q.id.as_str())),
            lines(&check),
        ));

        // complete_q_<quest>
        let mut done: Vec<String> = Vec::new();
        done.push(format!(
            "scoreboard players set {} {} 1",
            plan::PARTY,
            quest_score(q.id.as_str())
        ));
        done.extend(emit_effect_bundle(
            plan,
            &q.on_complete,
            root_audience(delvewright_dsl::EffectRootKind::QuestComplete),
        ));
        // activate quests triggered by this quest's completion
        for dep in &c.quests.content.quests {
            if let Trigger::QuestComplete { quest } = &dep.trigger
                && quest.as_str() == q.id.as_str()
            {
                done.push(format!(
                    "scoreboard players set {} {} 1",
                    plan::PARTY,
                    quest_active_score(dep.id.as_str())
                ));
            }
        }
        fns.push((
            format!("complete_q_{}", plan::safe_local(q.id.as_str())),
            lines(&done),
        ));
    }
    fns
}

/// `campaign_complete`, shared by every `campaign-complete` effect.
pub(super) fn campaign_complete_fns(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();
    let title = &c.world.content.title;
    let mut cc: Vec<String> = Vec::new();
    // Campaign completion is the party's (spec-0018): one holder write, and the
    // advancement + fanfare granted to every member — the delve ends for all of
    // them at once, whoever struck the last blow.
    cc.push(format!(
        "scoreboard players set {} dw.campaign 1",
        plan::PARTY
    ));
    cc.push(format!("advancement grant @a only {ns}:campaign_complete"));
    cc.push(format!(
        "tellraw @a {}",
        json!([
            tr_with(
                &chrome.get(delvewright_dsl::chrome::CAMPAIGN_COMPLETE),
                &[
                    ("color", json!("gold")),
                    ("with", json!([tr(title)])),
                ],
            ),
            { "text": "\n" },
            tr_with(
                &chrome.get(delvewright_dsl::chrome::CAMPAIGN_SIGNATURE),
                &[("color", json!("gray"))],
            )
        ])
    ));
    // v0.3 finale fanfare (M2 fix 4): the owner finished the finale and got no
    // feedback. Show a proper title banner + play a fanfare. Gated on v0.3 so the
    // shared `campaign_complete` stays byte-identical for hello-world / keep-crawl.
    cc.push(format!(
        "title @a title {}",
        tr_with(
            &chrome.get(delvewright_dsl::chrome::CAMPAIGN_BANNER),
            &[("color", json!("gold")), ("bold", json!(true))],
        )
    ));
    cc.push(format!(
        "title @a subtitle {}",
        tr_with(title, &[("color", json!("yellow"))])
    ));
    cc.push("playsound minecraft:ui.toast.challenge_complete player @a".to_string());
    // Machine-readable completion marker for the validation bot. The bot reads
    // `dw.campaign` from the sidebar per the amended contract, BUT mineflayer
    // 4.37.x cannot parse 1.21.11 scoreboard score packets (verified live: no
    // score updates ever surface). Broadcasting a stable token in chat — which
    // mineflayer DOES parse reliably — lets the bot observe completion. Same
    // anchored grammar as the per-objective markers, with the `campaign` token;
    // the harness treats its arrival anywhere before the final step as a hard
    // error (branch incoherence: the campaign completed while steps remained).
    // `@a` so a bot filling a seat in a future multiplayer delve still sees it.
    cc.push(format!(
        "tellraw @a {}",
        json!({
            "text": plan::marker_line(ns, plan::MARKER_TOKEN_CAMPAIGN),
            "color": "dark_gray"
        })
    ));
    fns.push(("campaign_complete".to_string(), lines(&cc)));
    fns
}
