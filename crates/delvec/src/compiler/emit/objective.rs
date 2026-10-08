//! Objectives: activation, completion cleanup, and the interact and reach tags.

use super::*;

/// Objective id → function-name-safe token (`obj/talk` → `o_talk`).
pub(super) fn safe_obj_fn(obj_id: &str) -> String {
    format!("o_{}", plan::safe_local(obj_id))
}

/// The entity tag on an `interact` objective's interaction hitbox.
pub(super) fn interact_entity_tag(obj_id: &str) -> String {
    format!("dw_i_{}", plan::safe_local(obj_id))
}

/// The entity tag on a `reach-anchor` objective's visual marker display.
pub(super) fn reach_marker_tag(obj_id: &str) -> String {
    format!("dw_r_{}", plan::safe_local(obj_id))
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
/// `collect` chest + item fill, an `interact` hitbox + glowing lantern marker, or a
/// `reach` glowing end-rod marker. Empty for objectives with no prop (talk-to,
/// kill) or an unresolvable anchor — both the `tick` activation driver and the
/// `activate_<obj>` function key off this being non-empty, so they never diverge.
pub(super) fn activation_commands(plan: &Plan, area: &str, o: &Objective) -> Vec<String> {
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
            // v0.8: an ADOPTED container is prefab furniture standing
            // in the room already — fill it where it stands, place nothing. Absent
            // `container`, the compiler keeps conjuring its own chest at the
            // anchor exactly as it always has. The adopted cell comes from
            // `plan.collect_fills`, the same resolution `DW0438` proved.
            let adopted = plan
                .collect_fills
                .iter()
                .find(|f| f.objective_id == id.as_str())
                .map(|f| f.cell);
            let Some(pos) = adopted.or_else(|| plan.point(area, anchor.as_str())) else {
                return cmds;
            };
            if adopted.is_none() {
                cmds.push(format!(
                    "setblock {} {} {} minecraft:chest",
                    pos[0], pos[1], pos[2]
                ));
            }
            // The objective's own stack lands in `container.0`; each padding stack
            // repeats it in the slots after it, so the container READS full
            // (vanilla fullness is occupied slots, not stack size). Positional and
            // total — no RNG, nothing to reseed (ADR-0006). A campaign with
            // neither a name nor padding emits the single pre-0.8 line, byte for
            // byte.
            let stack = format!(
                "{item}{} {count}",
                item_component_tail(item, item_name.as_deref())
            );
            for slot in 0..=*fill_count {
                cmds.push(format!(
                    "item replace block {} {} {} container.{slot} with {stack}",
                    pos[0], pos[1], pos[2]
                ));
            }
        }
        Objective::Interact { id, anchor, .. } => {
            if let Some(pos) = plan.point(area, anchor.as_str()) {
                // spec-0093 §6.5: a prop a hand presses IS the thing. The block
                // is placed and nothing else: no hitbox, no marker — vanilla's
                // `default_block_use` at this cell completes the objective.
                if let Some(block) = crate::compiler::pressable::interact_block(o) {
                    cmds.push(format!(
                        "setblock {} {} {} {}",
                        pos[0], pos[1], pos[2], block
                    ));
                    return cmds;
                }
                let e = ent_xyz(pos);
                cmds.push(format!(
                    "summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{}\"]}}",
                    e[0], e[1], e[2], interact_entity_tag(id.as_str())
                ));
                if let Some(prop) = o.prop() {
                    // v0.4: the prop block IS the affordance (spec-0008 §2) — place
                    // it at the anchor. No hologram marker: the block is visible.
                    cmds.push(format!(
                        "setblock {} {} {} {}",
                        pos[0], pos[1], pos[2], prop.block
                    ));
                } else if o.marker_shown(&plan.campaign.quests.content.guidance) {
                    // Visible, glowing, adventure-safe marker so a human can find the
                    // interact target (M2 fix 3): an `item_display` has no collision,
                    // so it obstructs neither movement nor the interaction hitbox.
                    // Named from the objective `title`; an untitled objective gets a
                    // nameless (but still glowing) marker rather than a raw-id label.
                    // Summoned only for a MARKED objective (spec-0093): an objective
                    // whose `marker` — or the campaign's `guidance.markers` — is
                    // `hidden` keeps the hitbox and shows nothing.
                    let name_fields = marker_name_fields(o.title());
                    cmds.push(format!(
                        "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_marker\",\"{}\"],{}billboard:\"center\",item:{{id:\"minecraft:lantern\",count:1}}}}",
                        e[0], e[1], e[2], interact_entity_tag(id.as_str()), name_fields
                    ));
                }
            }
        }
        Objective::ReachAnchor { id, anchor, .. } => {
            let pos = match plan
                .anchors
                .get(&(area.to_string(), anchor.as_str().to_string()))
            {
                Some(ResolvedAnchor::Point { pos, .. }) => *pos,
                Some(ResolvedAnchor::Gate { from, .. }) => *from,
                None => return cmds,
            };
            // A distinct, thematically neutral `end_rod` (vs. the interact lantern)
            // so a beacon-like light marks a reach destination. Named from the
            // objective `title`; untitled → nameless glow, never a raw-id label.
            // Summoned only for a MARKED objective (spec-0093); the completion
            // volume is adjudicated either way.
            if !o.marker_shown(&plan.campaign.quests.content.guidance) {
                return cmds;
            }
            let name_fields = marker_name_fields(o.title());
            let e = ent_xyz(pos);
            cmds.push(format!(
                "summon minecraft:item_display {} {} {} {{Glowing:1b,Tags:[{FIXTURE_NBT}\"dw_marker\",\"{}\"],{}billboard:\"center\",item:{{id:\"minecraft:end_rod\",count:1}}}}",
                e[0], e[1], e[2], reach_marker_tag(id.as_str()), name_fields
            ));
        }
        Objective::TalkTo { .. } | Objective::Kill { .. } => {}
    }
    cmds
}

/// The despawn commands run when an objective COMPLETES: remove every
/// entity its [`activation_commands`] summoned. The objective-scoped tag
/// (`dw_i_<obj>` on an interact's hitbox and its wayfinding marker, `dw_r_<obj>` on
/// a reach marker) is deterministic and unique to the objective, so a single tight
/// `kill @e[tag=…]` covers all of them without touching players (players never
/// carry these tags) or any other objective's markers. Interact-with-prop summons
/// only the hitbox (the prop is a block, not tagged); interact-without-prop and
/// reach also summon a `dw_marker` item_display carrying the same objective tag.
/// Prop BLOCKS and collect chests are the affordance itself and intentionally
/// persist as scenery — they are not entities and are not killed here. `collect`
/// (chest block only), `talk-to` and `kill` summon no per-objective entity, so they
/// contribute nothing.
pub(super) fn completion_cleanup(o: &Objective) -> Vec<String> {
    match o {
        Objective::Interact { id, .. } => {
            vec![format!("kill @e[tag={}]", interact_entity_tag(id.as_str()))]
        }
        Objective::ReachAnchor { id, .. } => {
            vec![format!("kill @e[tag={}]", reach_marker_tag(id.as_str()))]
        }
        Objective::Collect { .. } | Objective::TalkTo { .. } | Objective::Kill { .. } => Vec::new(),
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
