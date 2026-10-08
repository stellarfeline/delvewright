//! The cast: dispatch, selectors and barks.

use super::*;

/// The per-player scene selector the cast ledger dispatches on (spec-0020).
pub(super) const CAST_SCORE: &str = "dw.cast";

/// The right-click body for one NPC: the cast ledger's scene dispatch, or — for
/// an NPC no quest casts — the single `show_node_cmd(root)` line that has always
/// been there (so a campaign with no ledger is byte-identical).
///
/// ## Why re-evaluated rather than latched
///
/// `dw.qa_<quest>` is set to 1 when a quest starts and is never cleared, so it
/// reads "has begun". Emitting the selector clauses in quest-DAG order therefore
/// makes the **latest begun** quest win, and keep winning: the scene advances
/// with the story and never falls back. That is the whole retirement mechanism —
/// after the escape beat opens, Perimedes's right-click resolves to the escape
/// scene's root, and the premise root is unreachable *because the ledger says so*,
/// not because an author remembered a flag.
///
/// Scene `0` is "no declaring quest has begun yet" and shows the stage-6 root.
/// A `"none"` scene emits no action clause at all: the interaction advancement is
/// still granted and revoked one line above (the record is written and consumed),
/// and nothing opens.
pub(super) fn cast_dispatch(
    plan: &Plan,
    npc: &plan::NpcPlan,
    casts: &std::collections::BTreeMap<String, crate::compiler::cast::NpcCast>,
) -> Vec<String> {
    use crate::compiler::cast::SceneAction;
    let ns = &plan.namespace;
    let Some(cast) = casts.get(&npc.npc_id) else {
        return vec![show_node_cmd(plan, npc, &npc.root)];
    };
    let mut out = vec![format!("function {ns}:cast_{}", npc.safe)];
    out.push(format!(
        "execute if score @s {CAST_SCORE} matches 0 run {}",
        show_node_cmd(plan, npc, &npc.root)
    ));
    for scene in &cast.scenes {
        let i = scene.index;
        match &scene.action {
            SceneAction::Root(root) => out.push(format!(
                "execute if score @s {CAST_SCORE} matches {i} run {}",
                show_node_cmd(plan, npc, root)
            )),
            SceneAction::Barks(_) => out.push(format!(
                "execute if score @s {CAST_SCORE} matches {i} run function {ns}:bark_{}_{i}",
                npc.safe
            )),
            // Declared silence: no clause. The click is still recorded and
            // consumed by the `advancement revoke` above.
            SceneAction::Silent => {}
        }
    }
    out
}

/// The `cast_<npc>` selector function: compute which scene governs right now into
/// the per-player `dw.cast`.
///
/// Split out of `talk_<npc>` for the same reason `dmask_<npc>_<node>` is split out
/// of `show_<npc>_<node>`: it is pure scoreboard math, so a PackTest can drive it
/// and assert which scene the ledger selected **without opening a dialog** (a
/// PackTest dummy has no client to show a screen to).
pub(super) fn cast_selector_fn(
    plan: &Plan,
    npc: &plan::NpcPlan,
    casts: &std::collections::BTreeMap<String, crate::compiler::cast::NpcCast>,
) -> Option<(String, String)> {
    let cast = casts.get(&npc.npc_id)?;
    let mut body = vec![format!("scoreboard players set @s {CAST_SCORE} 0")];
    for cl in &cast.by_quest {
        // Per-branch casts add their branch gate to the same clause, so a
        // branch-divergent NPC really does dispatch per branch. Flags are party
        // state (`#party`), matching every other flag read in the dialogue path.
        let mut gate = String::new();
        for f in &cl.requires_flags {
            gate.push_str(&format!(
                " if score {} {} matches 1",
                plan::PARTY,
                plan::flag_score(f)
            ));
        }
        for f in &cl.forbids_flags {
            gate.push_str(&format!(
                " unless score {} {} matches 1",
                plan::PARTY,
                plan::flag_score(f)
            ));
        }
        // DSL v0.10 (spec-0031): the placement's numeric terms. The cast selector
        // is per-player (`dw.cast` on `@s`), so a `player`-scoped datum is legal
        // here too.
        gate.push_str(&state_cond(plan, &cl.requires_state, false));
        body.push(format!(
            "execute if score {} {} matches 1{gate} run scoreboard players set @s {CAST_SCORE} {}",
            plan::PARTY,
            quest_active_score(&cl.quest),
            cl.scene
        ));
    }
    Some((format!("cast_{}", npc.safe), lines(&body)))
}

/// One `bark_<npc>_<scene>` function per bark-pool scene: speak the next line and
/// advance the pool.
///
/// The counter is a `#bk_<npc>_<scene>` fake player on the shared `dw.sys`
/// objective — the repo's existing per-entity counter idiom — and it cycles by an
/// explicit clause ladder, so there is no RNG anywhere near a delve and the
/// n-th right-click always yields the same line.
pub(super) fn cast_bark_fns(
    plan: &Plan,
    npc: &plan::NpcPlan,
    casts: &std::collections::BTreeMap<String, crate::compiler::cast::NpcCast>,
) -> Vec<(String, String)> {
    use crate::compiler::cast::SceneAction;
    let mut out = Vec::new();
    let Some(cast) = casts.get(&npc.npc_id) else {
        return out;
    };
    let name = plan
        .campaign
        .npcs
        .content
        .npcs
        .iter()
        .find(|n| n.id.as_str() == npc.npc_id)
        .map(|n| n.name.clone())
        .unwrap_or_default();
    for scene in &cast.scenes {
        let SceneAction::Barks(pool) = &scene.action else {
            continue;
        };
        let holder = format!("#bk_{}_{}", npc.safe, scene.index);
        let mut body = vec![
            format!("scoreboard players add {holder} dw.sys 1"),
            format!(
                "execute if score {holder} dw.sys matches {}.. run scoreboard players set {holder} dw.sys 1",
                pool.len() + 1
            ),
        ];
        for (i, line) in pool.iter().enumerate() {
            let comp = json!([
                tr_with(&name, &[("color", json!("yellow"))]),
                { "text": ": " },
                tr_with(line, &[("italic", json!(true))])
            ]);
            body.push(format!(
                "execute if score {holder} dw.sys matches {} run tellraw @s {comp}",
                i + 1
            ));
        }
        out.push((format!("bark_{}_{}", npc.safe, scene.index), lines(&body)));
    }
    out
}
