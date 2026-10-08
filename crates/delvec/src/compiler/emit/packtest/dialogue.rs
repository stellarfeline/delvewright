use super::*;

/// The dialogue-trigger re-arm PackTest: a player consumes a dialogue trigger and
/// must be able to use it again **with the tick function never running in
/// between**. Suppressing the tick function is how a plain mcfunction emulates the
/// integrated (singleplayer) server's pause-menu tick freeze (1.21.9+), which is
/// the only condition under which the old per-tick-only re-enable lost a dialogue
/// choice — and which a dedicated server, and therefore every rung of the
/// validation ladder, can never enter.
///
/// Drives a **terminal** option (no `next`, no flag gate) so the handler contains
/// no `dialog show` — a PackTest dummy player has no client to show a screen to.
/// The re-arm is emitted immediately after the trigger reset, so it is reached on
/// every path through the handler regardless. Emits nothing when the campaign has
/// no terminal option (nothing to drive).
pub(super) fn emit_dialogue_trigger_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some((npc, opt)) = plan.npcs.iter().find_map(|npc| {
        npc.options
            .iter()
            .find(|o| o.next.is_none() && o.requires_flags.is_empty() && o.forbids_flags.is_empty())
            .map(|o| (npc, o))
    }) else {
        return;
    };
    let trig = &npc.trigger_objective;
    let n = opt.n;

    let (pin, sel) = pin_dummy("dw_t_rearm");
    let mut b = packtest_header(&format!(
        "{title}: dialogue trigger re-arms without a tick (singleplayer pause parity)"
    ));
    b.push(format!("function {ns}:setup"));
    // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it alone.
    b.push(pin);
    b.push("# The per-tick re-enable, run ONCE. Nothing below runs the tick".to_string());
    b.push("# function again: that suppression IS the integrated server's".to_string());
    b.push("# pause-menu tick freeze, which a dedicated server never enters.".to_string());
    b.push(format!("scoreboard players enable {sel} {trig}"));
    b.push(format!("execute as {sel} run trigger {trig} set {n}"));
    b.push(format!("assert score {sel} {trig} matches {n}"));
    b.push("# The tick's dispatch, hand-run: the handler consumes (and locks) the".to_string());
    b.push("# trigger, then must re-arm it itself.".to_string());
    b.push(format!(
        "execute as {sel} run function {ns}:dlg_{}_{n}",
        npc.safe
    ));
    b.push("# Second use, still with no tick in between. If the handler did not".to_string());
    b.push("# re-arm, vanilla rejects this and the score stays unset.".to_string());
    b.push(format!("execute as {sel} run trigger {trig} set {n}"));
    b.push(format!("assert score {sel} {trig} matches {n}"));

    out.insert(
        format!("packtest-datapack/data/{ns}/test/dialogue_trigger_rearm.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// One dialogue node's availability mask, driven per option and per axis.
///
/// The generalisation that made this a loop is worth stating, because the shape
/// it replaces is the one `crate::compiler::watch` exists to catch. The old walk searched
/// every node for the FIRST option that completes an objective, tested that one,
/// and stopped — so a node gated on a flag, on a FORBIDDEN flag or on a runtime
/// datum was not merely untested, it was unreachable by construction: the search
/// that skipped it was the same search that chose the exemplar. Reading the
/// emitted bytes said "one of two masks is driven", which is true and does not
/// say why.
///
/// So the axis is no longer chosen. Every gated option of the node is driven
/// through its OWN full display condition — flags required, flags forbidden, the
/// v0.10 numeric datum, and each completed objective's quest-active/not-yet-done
/// pair — asserted displayed, and then each term of that condition is broken on
/// its own and the bit asserted gone. Every axis the DSL has is covered because
/// the drive is written from the option's condition rather than from a list of
/// axes someone enumerated.
///
/// The bit is always ISOLATED (`(dw.dmask >> i) & 1`) rather than compared whole:
/// a node's options can share a quest-active score, so lighting one condition can
/// light several bits, and comparing the whole mask would read a sibling's bit as
/// this option's.
pub(super) fn emit_one_dialogue_mask_packtest(
    plan: &Plan,
    npc: &plan::NpcPlan,
    node_id: &str,
    gated: &[&plan::OptionPlan],
    out: &mut BuildOutput,
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let p = plan::PARTY;
    let node_safe = plan::safe_local(node_id);
    let key = format!("{}_{}", npc.safe, node_safe);
    let dmask = format!("{ns}:dmask_{key}");
    let (pin, sel) = pin_dummy(&format!("dw_t_dvis_{key}"));

    let mut bt = packtest_header(&format!(
        "{}: NPC `{}` node `{node_id}` — every gated option is displayed exactly when its own \
         condition holds",
        artifact_title(c),
        npc.npc_id
    ));
    bt.push(format!("function {ns}:setup"));
    // Pin this test's own dummy (see `pin_dummy`): with one dummy PER test
    // coexisting on the batch server, an `as @a` mask run + copy would read the
    // LAST dummy the selector visits — a foreign one.
    bt.push(pin);

    // Run the mask, then isolate one option's bit before the assert:
    // `(dw.dmask >> bit) & 1` via `%= 2^(bit+1)` then `/= 2^bit`.
    let assert_bit = |bt: &mut Vec<String>, bit: usize, present: bool| {
        bt.push(format!("execute as {sel} run function {dmask}"));
        // Copy the pinned dummy's mask into a fake player. `as {sel}` keeps the
        // read single-entity (`= @s …`): `scoreboard players get`/`operation`
        // reject a multi-entity selector.
        bt.push(format!(
            "execute as {sel} run scoreboard players operation #dm_{key} dw.sys = @s dw.dmask"
        ));
        bt.push(format!(
            "scoreboard players set #dmhi_{key} dw.sys {}",
            1u32 << (bit + 1)
        ));
        bt.push(format!(
            "scoreboard players operation #dm_{key} dw.sys %= #dmhi_{key} dw.sys"
        ));
        bt.push(format!(
            "scoreboard players set #dmlo_{key} dw.sys {}",
            1u32 << bit
        ));
        bt.push(format!(
            "scoreboard players operation #dm_{key} dw.sys /= #dmlo_{key} dw.sys"
        ));
        bt.push(format!(
            "assert score #dm_{key} dw.sys matches {}",
            u32::from(present)
        ));
    };

    for (bit, o) in gated.iter().enumerate() {
        // Satisfy this option's WHOLE display condition, term by term, from the
        // option itself — never from a hand-listed set of axes.
        let mut satisfied: Vec<String> = Vec::new();
        // Each term, with the line that BREAKS it and the line that restores it.
        // A term whose break and restore are the same shape is what lets the
        // negative half be written once for every axis the DSL has.
        let mut terms: Vec<(Vec<String>, Vec<String>)> = Vec::new();
        for f in &o.requires_flags {
            let sc = plan::flag_score(f);
            satisfied.push(format!("scoreboard players set {p} {sc} 1"));
            terms.push((
                vec![format!("scoreboard players set {p} {sc} 0")],
                vec![format!("scoreboard players set {p} {sc} 1")],
            ));
        }
        for f in &o.forbids_flags {
            let sc = plan::flag_score(f);
            satisfied.push(format!("scoreboard players set {p} {sc} 0"));
            terms.push((
                vec![format!("scoreboard players set {p} {sc} 1")],
                vec![format!("scoreboard players set {p} {sc} 0")],
            ));
        }
        // Every numeric term of the WHOLE condition — the option's own and each
        // completed objective's pending guard — is satisfied at once: two terms on
        // one datum (`at-most 5` on the option, `at-most 9` on its objective) are
        // driven to one value meeting both, never each to its own boundary with the
        // later write undoing the earlier.
        let all_cmps: Vec<&StateCompare> = o
            .requires_state
            .iter()
            .chain(
                o.completes
                    .iter()
                    .filter_map(|obj| objective_quest(c, obj))
                    .flat_map(|(_, objective)| objective.requires_state().iter()),
            )
            .collect();
        let joint = JointStateDrive::of(&all_cmps);
        joint.drive(plan, &o.requires_state, &mut satisfied, &mut terms);
        for obj in &o.completes {
            let Some((qid, objective)) = objective_quest(c, obj) else {
                continue;
            };
            let qa = quest_active_score(qid);
            let os = obj_score(obj);
            satisfied.push(format!("scoreboard players set {p} {qa} 1"));
            satisfied.push(format!("scoreboard players set {p} {os} 0"));
            // The objective-state axis is the objective's whole pending guard
            // (spec-0093 §6.3), and every term of it hides the option on its
            // own: the quest is not running, the objective is already done, a
            // beat it declares `after` is not done, a flag it requires is unset,
            // a flag it forbids is set, a datum it reads does not satisfy it.
            terms.push((
                vec![format!("scoreboard players set {p} {qa} 0")],
                vec![format!("scoreboard players set {p} {qa} 1")],
            ));
            terms.push((
                vec![format!("scoreboard players set {p} {os} 1")],
                vec![format!("scoreboard players set {p} {os} 0")],
            ));
            for a in objective.after() {
                let sc = obj_score(a.as_str());
                satisfied.push(format!("scoreboard players set {p} {sc} 1"));
                terms.push((
                    vec![format!("scoreboard players set {p} {sc} 0")],
                    vec![format!("scoreboard players set {p} {sc} 1")],
                ));
            }
            for f in objective.requires_flags() {
                let sc = plan::flag_score(f.as_str());
                satisfied.push(format!("scoreboard players set {p} {sc} 1"));
                terms.push((
                    vec![format!("scoreboard players set {p} {sc} 0")],
                    vec![format!("scoreboard players set {p} {sc} 1")],
                ));
            }
            for f in objective.forbids_flags() {
                let sc = plan::flag_score(f.as_str());
                satisfied.push(format!("scoreboard players set {p} {sc} 0"));
                terms.push((
                    vec![format!("scoreboard players set {p} {sc} 1")],
                    vec![format!("scoreboard players set {p} {sc} 0")],
                ));
            }
            joint.drive(plan, objective.requires_state(), &mut satisfied, &mut terms);
        }
        if terms.is_empty() {
            continue;
        }
        // Displayed when its own condition holds…
        bt.extend(satisfied.iter().cloned());
        assert_bit(&mut bt, bit, true);
        // …and gone the moment any single term of it stops holding. One term at a
        // time, restored after: a template that broke them all at once would pass
        // just as well against a mask that reads only the first.
        for (break_it, restore) in &terms {
            bt.extend(break_it.iter().cloned());
            assert_bit(&mut bt, bit, false);
            bt.extend(restore.iter().cloned());
        }
    }

    out.insert(
        format!("packtest-datapack/data/{ns}/test/dlg_mask_{key}.mcfunction"),
        lines(&bt).into_bytes(),
    );
}

/// The claim the dialogue-mask loop makes, judged by `DW0811`. One claim per
/// NPC, because the family prefix carries the NPC's own id (`dmask_<npc>_`) —
/// which is why `Claim::families` is owned rather than a static list.
///
/// `declared` is every node the NPC's authored options mention, so a node whose
/// options are all ungated (and which therefore emits no `dmask_` body at all) is
/// declared and not judged, while a gated node the loop skipped is a breach.
pub(super) fn dialogue_mask_watch_claims(plan: &Plan) -> Vec<crate::compiler::watch::Claim> {
    plan.npcs
        .iter()
        .map(|npc| crate::compiler::watch::Claim {
            mechanic: "dialogue-mask",
            families: vec![format!("dmask_{}_", npc.safe)],
            declared: npc
                .options
                .iter()
                .map(|o| plan::safe_local(&o.node_id))
                .collect(),
        })
        .collect()
}
