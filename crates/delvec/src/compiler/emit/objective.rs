//! Objectives: activation, completion cleanup, and the interact tag.

use super::*;

/// Objective id → function-name-safe token (`obj/talk` → `o_talk`).
pub(super) fn safe_obj_fn(obj_id: &str) -> String {
    format!("o_{}", plan::safe_local(obj_id))
}

/// The entity tag on an `interact` objective's interaction hitbox.
pub(super) fn interact_entity_tag(obj_id: &str) -> String {
    format!("dw_i_{}", plan::safe_local(obj_id))
}

/// Scoreboard (dummy, `dw.sys`) holder for the "already holding the item" per-tick
/// collect completion check (gap 13).
pub(super) const COLLECT_HOLD: &str = "dw.hold";

/// The fake-player sentinel on `dw.sys` that guards an objective's activation
/// placement so it runs exactly once, world-wide (gap 13).
pub(super) fn activation_flag(obj_id: &str) -> String {
    format!("#act_{}", plan::safe_local(obj_id))
}

/// Whether the campaign declares any `collect` objective (gates the `dw.hold`
/// scratch declaration so campaigns without collect stay byte-identical).
pub(super) fn has_collect_objective(c: &delvewright_dsl::Campaign) -> bool {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .any(|o| matches!(o, Objective::Collect { .. }))
}

/// The world-placement commands run when an objective ACTIVATES (gap 13): a
/// `collect` chest + item fill, or an `interact`'s prop and hitbox. Empty for
/// objectives that place nothing (`talk-to`, `kill`, `reach-anchor`) or an
/// unresolvable anchor — both the `tick` activation driver and the
/// `activate_<obj>` function key off this being non-empty, so they never diverge.
pub(super) fn activation_commands(
    plan: &Plan,
    holders: &[crate::compiler::holders::OwnedHolder],
    area: &str,
    o: &Objective,
) -> Vec<String> {
    let mut cmds = Vec::new();
    match o {
        Objective::Collect {
            id,
            item,
            count,
            anchor,
            item_name,
            fill_count,
            dropped_by,
            ..
        } => {
            // v0.9: a drop-gated collect is provisioned by the fight,
            // not by the world. Place nothing and fill nothing — the item exists
            // only once the boss dies, which is exactly what makes the chain
            // provable (`DW0493`) instead of merely intended.
            if dropped_by.is_some() {
                return cmds;
            }
            // spec-0111 §3.4: an adopted holder ENTITY — a frame or a stand
            // the piece carries — is filled by `item replace entity` in its
            // first slot, a frame unfrozen to give it up; the engine summoned
            // the body at `setup_finish`.
            let stack = format!(
                "{item}{} {count}",
                item_component_tail(item, item_name.as_deref())
            );
            if let Some(lines) = collect_holder_activation(holders, id.as_str(), &stack) {
                return lines;
            }
            // v0.8: an ADOPTED container is prefab furniture standing
            // in the room already — fill it where it stands, place nothing. Absent
            // `holder`, the compiler keeps conjuring its own chest at the
            // anchor exactly as it always has. The adopted cell comes from
            // `plan.collect_fills`, the same resolution `DW0438` proved.
            let adopted = plan
                .collect_fills
                .iter()
                .find(|f| f.objective_id == id.as_str());
            let Some(pos) = adopted
                .map(|f| f.cell)
                .or_else(|| plan.point(area, anchor.as_str()))
            else {
                return cmds;
            };
            // The conjured chest, from the one list the region model reads.
            if let Some((p, block)) = plan::activation_prop(&plan.anchors, area, o) {
                cmds.push(format!("setblock {} {} {} {block}", p[0], p[1], p[2]));
            }
            // The objective's own stack lands in the slot the collect names
            // (`container.0` when it names none); each padding stack repeats it
            // in the slots after it, so the container READS full (vanilla
            // fullness is occupied slots, not stack size). Positional and total
            // — no RNG, nothing to reseed (ADR-0006). The adopted fill's slots
            // are the holder class's one landing rule (spec-0111 §3.4).
            let slots: Vec<String> = adopted.map_or_else(
                || {
                    (0..=*fill_count)
                        .map(|n| format!("container.{n}"))
                        .collect()
                },
                |f| {
                    crate::compiler::holders::collect_slots(
                        delvewright_dsl::holders::HolderFamily::Container,
                        f,
                    )
                },
            );
            for slot in slots {
                cmds.push(format!(
                    "item replace block {} {} {} {slot} with {stack}",
                    pos[0], pos[1], pos[2]
                ));
            }
        }
        Objective::Interact { id, anchor, .. } => {
            if let Some(pos) = plan.point(area, anchor.as_str()) {
                // spec-0093 §6.5: a prop a hand presses IS the thing. The block
                // is placed and nothing else: no hitbox, no marker — vanilla's
                // `default_block_use` at this cell completes the objective.
                let prop = plan::activation_prop(&plan.anchors, area, o);
                if crate::compiler::pressable::interact_block(o).is_some() {
                    if let Some((p, block)) = &prop {
                        cmds.push(format!("setblock {} {} {} {block}", p[0], p[1], p[2]));
                    }
                    return cmds;
                }
                // The standard column, bracketing the prop's cell and the one above
                // it: a full-cube prop flush with a `1.0`-wide box wins every click
                // that reaches its faces ([`crate::compiler::clickbox`]).
                cmds.push(
                    crate::compiler::clickbox::ClickBox::column(pos, 2).summon(&format!(
                        "response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{}\"]",
                        interact_entity_tag(id.as_str())
                    )),
                );
                if let Some((p, block)) = &prop {
                    // v0.4: the prop block IS the affordance (spec-0008 §2) — place
                    // it at the anchor. No hologram marker: the block is visible.
                    cmds.push(format!("setblock {} {} {} {block}", p[0], p[1], p[2]));
                }
                // A prop-less interact stands on a block the piece authored
                // (`DW0963` refuses one on air, spec-0104 §7.6): the hitbox
                // over it is the whole affordance.
            }
        }
        Objective::ReachAnchor { .. } | Objective::TalkTo { .. } | Objective::Kill { .. } => {}
    }
    cmds
}

/// The despawn commands run when an objective COMPLETES: remove every
/// entity its [`activation_commands`] summoned. The objective-scoped tag
/// (`dw_i_<obj>` on an interact's hitbox) is deterministic and unique to the
/// objective, so a single tight `kill @e[tag=…]` covers it without touching
/// players (players never carry these tags) or any other objective's entities.
/// Prop BLOCKS and collect chests are the affordance itself and intentionally
/// persist as scenery — they are not entities and are not killed here. `collect`
/// (chest block only), `talk-to`, `kill` and `reach-anchor` summon no
/// per-objective entity, so they contribute nothing.
pub(super) fn completion_cleanup(o: &Objective) -> Vec<String> {
    match o {
        Objective::Interact { id, .. } => {
            vec![format!("kill @e[tag={}]", interact_entity_tag(id.as_str()))]
        }
        Objective::ReachAnchor { .. }
        | Objective::Collect { .. }
        | Objective::TalkTo { .. }
        | Objective::Kill { .. } => Vec::new(),
    }
}

/// `(objective id, quest id)` for every `interact` objective, in declared order.
pub(super) fn interact_objectives(c: &delvewright_dsl::Campaign) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if matches!(o, Objective::Interact { .. }) {
                out.push((o.id().as_str().to_string(), q.id.as_str().to_string()));
            }
        }
    }
    out
}

/// The `interact` and `collect` reward functions their advancements run.
pub(super) fn reward_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut fns: Vec<(String, String)> = Vec::new();
    for q in &c.quests.content.quests {
        let qa = quest_active_score(q.id.as_str());
        for o in &q.objectives {
            match o {
                Objective::Interact { id, .. } => {
                    // Human click path: the interaction advancement sets the same
                    // trigger the bot chats; the per-tick handler applies guards.
                    fns.push((
                        format!("i_reward_{}", plan::safe_local(id.as_str())),
                        lines(&[
                            format!(
                                "advancement revoke @s only {ns}:i_{}",
                                plan::safe_local(id.as_str())
                            ),
                            format!(
                                "scoreboard players set @s {} 1",
                                plan::interact_trigger(id.as_str())
                            ),
                        ]),
                    ));
                }
                Objective::Collect { id, .. } => {
                    // inventory_changed reward: complete (if the quest/after/flags
                    // guards hold), then re-arm.
                    fns.push((
                        format!("c_reward_{}", plan::safe_local(id.as_str())),
                        lines(&[
                            format!(
                                "execute{} run function {ns}:complete_{}",
                                pending_guard(plan, o, &qa),
                                safe_obj_fn(id.as_str())
                            ),
                            format!(
                                "advancement revoke @s only {ns}:c_{}",
                                plan::safe_local(id.as_str())
                            ),
                        ]),
                    ));
                }
                _ => {}
            }
        }
    }
    fns
}
