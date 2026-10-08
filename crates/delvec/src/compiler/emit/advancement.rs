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

    // spec-0016 §1: one advancement per bonfire, so a
    // right-click opens the rest dialog AS the player who clicked. The interaction
    // entity's own `interaction` record cannot do this — it names no player the
    // `dialog show` could target — and this is the same vanilla criterion every
    // `interact` objective already runs on. `bonfire_open_<i>` revokes it, so the
    // bonfire is re-openable forever (a rest point is used, never consumed).
    for bf in plan.bonfires() {
        let i = bf.index;
        advs.push((
            format!("bf_{i}"),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"dw_bonfire_{i}\"]}}")
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:bonfire_open_{i}") }
            }),
        ));
    }

    // spec-0032: one advancement per shop, and one per stake. Both are the bonfire's
    // primitive verbatim — the interaction entity's own `interaction` record names
    // no player, and a shop has to know WHO is buying and a stake WHOSE wager is
    // being collected. Each handler revokes its own grant, so both are re-usable.
    for (i, _, _) in shops(plan) {
        advs.push((
            format!("shop_{i}"),
            json!({
                "criteria": {
                    "interact": {
                        "trigger": "minecraft:player_interacted_with_entity",
                        "conditions": {
                            "entity": {
                                "type": "minecraft:interaction",
                                "nbt": format!("{{Tags:[\"dw_shop_{i}\"]}}")
                            }
                        }
                    }
                },
                "rewards": { "function": format!("{ns}:shop_open_{i}") }
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
    // This is `seal_<safe>` lifted off `close-gate`. It keys on the trigger's own
    // `dw_trig_<id>` tag, which `seal_fns` / `ws_arm_fns` / `env_trigger_setup`
    // already put on whatever body that trigger rides or summons — so the
    // advancement needs to know nothing about seals, doors, or any future
    // pressable object class.
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
        let criterion = match block {
            Some((block, cell)) => block_use_criterion(&block, cell),
            None => json!({
                "trigger": "minecraft:player_interacted_with_entity",
                "conditions": {
                    "entity": {
                        "type": "minecraft:interaction",
                        "nbt": format!("{{Tags:[\"dw_trig_{id}\"]}}")
                    }
                }
            }),
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
                Objective::Interact { id, anchor, .. } => {
                    let tag = interact_entity_tag(id.as_str());
                    // spec-0093 §6.5: a block a hand presses is reported by
                    // vanilla's own `default_block_use` at the block's cell, as
                    // the player who pressed; everything else rides the hitbox.
                    let cell = plan
                        .quest_area(q.id.as_str())
                        .and_then(|a| plan.point(a, anchor.as_str()));
                    let criterion = match crate::compiler::pressable::interact_block(o).zip(cell) {
                        Some((block, cell)) => block_use_criterion(block, cell),
                        None => json!({
                            "trigger": "minecraft:player_interacted_with_entity",
                            "conditions": {
                                "entity": {
                                    "type": "minecraft:interaction",
                                    "nbt": format!("{{Tags:[\"{tag}\"]}}")
                                }
                            }
                        }),
                    };
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
