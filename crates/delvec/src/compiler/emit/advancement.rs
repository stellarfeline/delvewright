//! The advancements every right-clickable object and objective answers through.

use super::*;

pub(super) fn emit_advancements(
    plan: &Plan,
    chrome: &delvewright_dsl::Chrome,
    wave_placements: &WavePlacements,
) -> Vec<(String, Value)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut advs = Vec::new();

    // spec-0016 §1: one advancement per bonfire, so a press opens the rest
    // dialog AS the player who pressed. `bonfire_open_<i>` revokes it, so the
    // bonfire is re-openable forever (a rest point is used, never consumed).
    // spec-0117: the press is the block's own report where no box stands over
    // it in the open state, the box's otherwise.
    for bf in plan.bonfires() {
        let i = bf.index;
        let Some(p) = press_of_owner(
            plan,
            &crate::compiler::pressable::Owner::Bonfire { index: i },
        ) else {
            continue;
        };
        advs.push((
            format!("bf_{i}"),
            json!({
                "criteria": { "interact": press_criterion_of(p, crate::compiler::pressable::State::Open) },
                "rewards": { "function": format!("{ns}:bonfire_open_{i}") }
            }),
        ));
    }

    // spec-0032: one advancement per shop — the presser is who is buying.
    // spec-0117: reported by the shop's block or its box, as for a bonfire.
    for (i, _, _) in shops(plan) {
        let Some(p) = press_of_owner(plan, &crate::compiler::pressable::Owner::Shop { index: i })
        else {
            continue;
        };
        advs.push((
            format!("shop_{i}"),
            json!({
                "criteria": { "interact": press_criterion_of(p, crate::compiler::pressable::State::Open) },
                "rewards": { "function": format!("{ns}:shop_open_{i}") }
            }),
        ));
    }

    // spec-0117: an unlock or a disarm whose block reports its own press is
    // dispatched by that report, as the presser; a boxed one stays on the
    // tick's record poll.
    for sc in &plan.shortcuts {
        if unlock_by_box(plan, sc) {
            continue;
        }
        let Some(p) = press_of_owner(
            plan,
            &crate::compiler::pressable::Owner::Shortcut { id: sc.id.clone() },
        ) else {
            continue;
        };
        advs.push((
            format!("sc_{}", sc.safe),
            json!({
                "criteria": { "interact": block_use_criterion(&p.block, p.cell) },
                "rewards": { "function": format!("{ns}:sc_press_{}", sc.safe) }
            }),
        ));
    }
    for (kind, owner, safe) in plan
        .traps
        .iter()
        .filter(|t| t.disarm.is_some() && !disarm_by_box(plan, &t.id, false))
        .map(|t| {
            (
                "trapdis",
                crate::compiler::pressable::Owner::TrapDisarm { id: t.id.clone() },
                t.safe.clone(),
            )
        })
        .chain(
            plan.timed_gates
                .iter()
                .filter(|g| g.disarm.is_some() && !disarm_by_box(plan, &g.id, true))
                .map(|g| {
                    (
                        "tgdis",
                        crate::compiler::pressable::Owner::TimedGateDisarm { id: g.id.clone() },
                        g.safe.clone(),
                    )
                }),
        )
    {
        let Some(p) = press_of_owner(plan, &owner) else {
            continue;
        };
        advs.push((
            format!("{kind}_{safe}"),
            json!({
                "criteria": { "interact": block_use_criterion(&p.block, p.cell) },
                "rewards": { "function": format!("{ns}:{kind}_press_{safe}") }
            }),
        ));
    }
    // spec-0117: a trapped chest the trap names is its own detector.
    for t in &plan.traps {
        if t.payload_effects.is_empty() || t.trigger != delvewright_dsl::TrapTrigger::TrappedChest {
            continue;
        }
        advs.push((
            format!("trapfire_{}", t.safe),
            json!({
                "criteria": { "interact": block_use_criterion("minecraft:trapped_chest", t.trigger_cell) },
                "rewards": { "function": format!("{ns}:trap_open_{}", t.safe) }
            }),
        ));
    }
    // ONE advancement for every stake, because there is one box to click: a marker
    // is a place, and the place offers itself to every stake the death left there.
    // One per stake would grant several advancements for a single right-click on a
    // single entity, and vanilla grants an advancement at most once per tick — so
    // the wagers a player got back would depend on which grant the server got to.
    if stakes(plan).iter().any(|(st, _)| st.max_live() > 0) {
        let tag = stk_tag();
        advs.push((
            STK_COLLECT_FN.to_string(),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"{tag}\"]}}")
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:{STK_COLLECT_FN}") }
            }),
        ));
    }

    // DSL v0.11: one advancement per `audience: presser` trigger, so a right-click
    // on the thing runs its bundle AS the player who pressed it. `press_<id>`
    // revokes its own grant, so the object answers every press — a wall is not
    // consumed by being asked.
    //
    // It keys on the trigger's own `dw_trig_<id>` tag, which every click box,
    // NPC hitbox and answer box a trigger rides wears (`pressable::box_tags`) —
    // so the advancement needs to know nothing about seals, doors, or any
    // future pressable object class.
    for t in &plan.emitted_triggers(chrome) {
        // spec-0093 §6.5: a trigger whose prop a hand presses is dispatched by
        // vanilla's `default_block_use` at the block's cell, whoever it
        // addresses — the block is the body and nothing is polled. A presser
        // `step` is dispatched from the tick (`step_trigger_poll`), and is no
        // click.
        let block = match crate::compiler::pressable::trigger_body(plan, t) {
            crate::compiler::pressable::Body::Block { cell, block } => Some((block, cell)),
            _ => None,
        };
        if block.is_none() && (!t.addresses_presser() || !t.on.is_click()) {
            continue;
        }
        let id = plan::safe_local(t.id.as_str());
        // The press kind names its own criterion (`press_criterion`): a
        // right-click `player_interacted_with_entity`, a left-click
        // `player_hurt_entity` (spec-0119), each keyed on the interaction the
        // press lands on (`trigger_carrier_tag`).
        let criterion = match (block, press_criterion(&t.on)) {
            (Some((block, cell)), _) => block_use_criterion(&block, cell),
            (None, Some(trigger)) => json!({
                "trigger": trigger,
                "conditions": {
                    "entity": {
                        "type": "minecraft:interaction",
                        "nbt": format!("{{Tags:[\"{}\"]}}", trigger_carrier_tag(t))
                    }
                }
            }),
            (None, None) => continue,
        };
        advs.push((
            format!("press_{id}"),
            json!({
                "criteria": { "interact": criterion },
                "rewards": { "function": format!("{ns}:press_{id}") }
            }),
        ));
    }

    // one interaction advancement per NPC
    for npc in &plan.npcs {
        advs.push((
            format!("{}_interact", npc.safe),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        // 1.21.11's `player_interacted_with_entity` `entity` field is
                        // an Either<single entity sub-predicate, list of loot
                        // conditions>. The list form requires each entity_properties
                        // condition to carry its own `entity: "this"` key; the single
                        // sub-predicate object form is simpler and is what loads
                        // cleanly on a live server (verified in the load shakeout —
                        // the list form failed with "No key entity in MapLike").
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"{}\"]}}", npc.tag)
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:talk_{}", npc.safe) }
            }),
        ));
    }

    // v0.3: one advancement per interact objective, collect objective and wave.
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            match o {
                Objective::Interact { id, .. } => {
                    // spec-0117 §5.2: the act is reported by the block itself
                    // where no box stands in the open state, by the box
                    // otherwise; either way as the player who pressed.
                    let Some(p) = crate::compiler::pressable::press_of_objective(plan, id.as_str())
                    else {
                        continue;
                    };
                    let criterion = press_criterion_of(p, crate::compiler::pressable::State::Open);
                    advs.push((
                        format!("i_{}", plan::safe_local(id.as_str())),
                        json!({
                            "criteria": { "interact": criterion },
                            "rewards": { "function": format!("{ns}:i_reward_{}", plan::safe_local(id.as_str())) }
                        }),
                    ));
                }
                Objective::Collect {
                    id, item, count, ..
                } => {
                    advs.push((
                        format!("c_{}", plan::safe_local(id.as_str())),
                        json!({
                            "criteria": {
                                "got": {
                                    "trigger": "minecraft:inventory_changed",
                                    "conditions": {
                                        "items": [ { "items": item, "count": { "min": count } } ]
                                    }
                                }
                            },
                            "rewards": { "function": format!("{ns}:c_reward_{}", plan::safe_local(id.as_str())) }
                        }),
                    ));
                }
                _ => {}
            }
        }
    }
    // One kill advancement per wave that has kill machinery — the same gate
    // `k_reward_<wave>` is emitted behind. A wave no beat seats resolves no
    // spawn area and gets no reward function, so an advancement for it would
    // name a function the pack never had (`DW0497` reads advancement rewards).
    for w in &c.quests.content.waves {
        if !wave_placements.contains_key(w.id.as_str()) {
            continue;
        }
        let tag = plan::wave_tag(w.id.as_str());
        advs.push((
            format!("k_{}", plan::safe_local(w.id.as_str())),
            json!({
                "criteria": {
                    "slain": {
                        "trigger": "minecraft:player_killed_entity",
                        "conditions": {
                            "entity": { "nbt": format!("{{Tags:[\"{tag}\"]}}") }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:k_reward_{}", plan::safe_local(w.id.as_str())) }
            }),
        ));
    }
    // spec-0074: an actor's kill advancement, emitted only for an actor that
    // declares `on_kill` — the same `player_killed_entity` criterion a wave's
    // `k_<wave>` uses, over the actor's own tag.
    for (a, _) in on_kill_actors(plan) {
        let safe = plan::safe_local(a.id.as_str());
        advs.push((
            format!("ka_{safe}"),
            json!({
                "criteria": {
                    "slain": {
                        "trigger": "minecraft:player_killed_entity",
                        "conditions": {
                            "entity": { "nbt": format!("{{Tags:[\"dw_actor_{safe}\"]}}") }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:ka_reward_{safe}") }
            }),
        ));
    }

    // campaign-complete advancement (granted by command). Both player-visible
    // strings are campaign-derived and therefore localized: `localize` rewrites the
    // whole `Campaign` before emission, so whatever we read here is already in the
    // target language. The description was a hardcoded `"You left the keep."` on
    // every delve ever built — the reference keep-crawl's line, shipped verbatim to
    // a shipwreck campaign and untranslatable in every sidecar because it never
    // passed through a `Campaign` field at all.
    let outro = campaign_outro(c);
    advs.push((
        "campaign_complete".to_string(),
        json!({
            "criteria": { "granted": { "trigger": "minecraft:impossible" } },
            "display": {
                "icon": { "id": "minecraft:iron_door" },
                "title": tr(&c.world.content.title),
                "description": tr(&outro),
                "frame": "goal",
                "show_toast": true,
                "announce_to_chat": false,
                "hidden": false
            }
        }),
    ));
    advs
}
