use super::*;
use crate::compiler::holders::OwnedHolder;
use delvewright_dsl::holders::HolderFamily;

/// **One PackTest per filled holder entity** (spec-0111 §5.3): `holder_<id>`
/// drives the holder's own body and asserts what the engine owns about it —
/// one body carries its tag, every slot a `loot` fill claims holds its item,
/// and the flags are the ones §4.1 sets — and, on a frame, the §4.3 freeze:
/// emptied, a `tick` later it is `Fixed` and `Invulnerable` — that `tick` run
/// with the load latched (`settled_setup`), so it judges the frame this test
/// emptied rather than a `setup_finish` that refills it. The test restores
/// the frame's contents and flags as it found them, so it leaves the world as
/// any sibling template expects it. A PackTest dummy performs no real attack
/// or click; the take is the bot's.
pub(super) fn emit_holder_packtests(plan: &Plan, out: &mut BuildOutput, holders: &[OwnedHolder]) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for o in holders.iter().filter(|o| o.is_filled()) {
        let id = &o.id;
        let tag = o.tag();
        let sel = o.selector();
        let mut b = packtest_header(&format!(
            "{title}: the `{}` `{id}` is summoned and filled as the engine owns it (spec-0111)",
            o.holder.kind.id
        ));
        // Latched: the freeze below runs one `tick`, which must judge the
        // frame this template emptied, not a load that refills it.
        b.extend(settled_setup(ns));
        b.push(format!("function {ns}:{}", super::super::holder_fn(o)));
        b.push(format!("scoreboard players set #hn_{id} dw.sys 0"));
        b.push(format!(
            "execute store result score #hn_{id} dw.sys if entity @e[tag={tag}]"
        ));
        b.push(format!("assert score #hn_{id} dw.sys matches 1"));
        for (k, f) in o.fills.iter().enumerate() {
            let Some(it) = &f.stack else { continue };
            b.push(format!("scoreboard players set #hs{k}_{id} dw.sys 0"));
            b.push(format!(
                "execute store success score #hs{k}_{id} dw.sys if items entity {sel} {} {}",
                f.slot, it.item
            ));
            b.push(format!("assert score #hs{k}_{id} dw.sys matches 1"));
        }
        // The owned flags, one assertion each, so a red names the key. A
        // marker stand is asserted absent rather than `Marker:0b` present: the
        // game may omit a false flag from the entity's saved NBT.
        let mut flags: Vec<(String, bool)> = Vec::new();
        match o.holder.kind.family {
            HolderFamily::Stand => {
                flags.push(("Invulnerable:1b".to_string(), true));
                flags.push((format!("DisabledSlots:{}", o.disabled_slots()), true));
                flags.push(("Marker:1b".to_string(), false));
                if o.show_arms() {
                    flags.push(("ShowArms:1b".to_string(), true));
                }
            }
            _ => flags.push(("ItemDropChance:1.0f".to_string(), true)),
        }
        for (k, (flag, present)) in flags.iter().enumerate() {
            let verb = if *present { "if" } else { "unless" };
            b.push(format!("scoreboard players set #hf{k}_{id} dw.sys 0"));
            b.push(format!(
                "execute store success score #hf{k}_{id} dw.sys {verb} entity @e[tag={tag},nbt={{{flag}}}]"
            ));
            b.push(format!("assert score #hf{k}_{id} dw.sys matches 1"));
        }
        if o.holder.kind.family == HolderFamily::Frame {
            let item = o
                .fills
                .iter()
                .find_map(|f| f.stack.as_ref().map(|s| s.item.clone()))
                .or_else(|| {
                    o.collect.as_ref().and_then(|obj| {
                        plan.campaign
                            .quests
                            .content
                            .quests
                            .iter()
                            .flat_map(|q| &q.objectives)
                            .find(|x| x.id().as_str() == obj)
                            .and_then(|x| match x {
                                delvewright_dsl::Objective::Collect { item, .. } => {
                                    Some(item.clone())
                                }
                                _ => None,
                            })
                    })
                })
                .unwrap_or_else(|| "minecraft:air".to_string());
            // Held before: an open frame (a loot fill, or an activated collect).
            b.push(format!("scoreboard players set #hb_{id} dw.sys 0"));
            b.push(format!(
                "execute store success score #hb_{id} dw.sys if items entity {sel} contents *"
            ));
            b.push(format!(
                "item replace entity {sel} contents with minecraft:air"
            ));
            b.push(format!("function {ns}:tick"));
            b.push(format!("scoreboard players set #hz_{id} dw.sys 0"));
            b.push(format!(
                "execute store success score #hz_{id} dw.sys if entity \
                 @e[tag={tag},nbt={{Fixed:1b,Invulnerable:1b}}]"
            ));
            b.push(format!("assert score #hz_{id} dw.sys matches 1"));
            b.push(format!(
                "execute if score #hb_{id} dw.sys matches 1 run item replace entity {sel} \
                 contents with {item}"
            ));
            b.push(format!(
                "execute if score #hb_{id} dw.sys matches 1 run data merge entity {sel} \
                 {{Fixed:0b,Invulnerable:0b}}"
            ));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/holder_{id}.mcfunction"),
            lines(&b).into_bytes(),
        );
        // A collect taking from the holder: its own reward, driven, completes
        // it — the guards satisfied on this test's own dummy, as `verb_collect`
        // drives the first collect's.
        if let Some(obj) = &o.collect
            && let Some((qid, objective)) =
                plan.campaign.quests.content.quests.iter().find_map(|q| {
                    q.objectives
                        .iter()
                        .find(|x| x.id().as_str() == obj)
                        .map(|x| (q.id.as_str(), x))
                })
        {
            let (pin, dummy) = pin_dummy(&format!("dw_t_h{}", plan::safe_local(obj)));
            let mut t = packtest_header(&format!(
                "{title}: taking `{obj}`'s stack off the `{}` completes it (spec-0111)",
                o.holder.kind.id
            ));
            t.push(format!("function {ns}:setup"));
            t.push(pin);
            t.push(format!(
                "scoreboard players set {} {} 0",
                plan::PARTY,
                obj_score(obj)
            ));
            t.extend(packtest_preamble(plan, qid, objective, true, &dummy));
            t.push(format!(
                "execute as {dummy} run function {ns}:c_reward_{}",
                plan::safe_local(obj)
            ));
            t.push(format!(
                "assert score {} {} matches 1",
                plan::PARTY,
                obj_score(obj)
            ));
            out.insert(
                format!("packtest-datapack/data/{ns}/test/holder_take_{id}.mcfunction"),
                lines(&t).into_bytes(),
            );
        }
    }
}

/// `DW0811`'s claim over the holders: every filled holder's own body
/// `holder_<id>` is driven by its template.
pub(super) fn holder_watch_claim(holders: &[OwnedHolder]) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "holder",
        kind: crate::compiler::watch::ClaimKind::Body,
        families: vec!["holder_".to_string()],
        declared: holders
            .iter()
            .filter(|o| o.is_filled())
            .map(|o| o.id.clone())
            .collect(),
    }
}
