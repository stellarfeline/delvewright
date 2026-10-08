//! Loops.

use super::*;

/// The declaration behind a resolved loop — its gate is read off the one
/// declaration rather than off a copy (spec-0086).
pub(super) fn loop_decl<'a>(
    plan: &Plan<'a>,
    l: &crate::compiler::r#loop::LoopPlan,
) -> Option<&'a delvewright_dsl::Loop> {
    plan.campaign
        .quests
        .content
        .loops
        .iter()
        .find(|d| d.id.as_str() == l.id)
}

/// The per-tick driver lines for the campaign's loops (spec-0086 §7), in
/// declaration order: one call into each loop's poll. Empty for a campaign that
/// declares none, so the emitted `tick` is byte-identical for everybody else.
pub(super) fn loop_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    plan.loops
        .iter()
        .map(|l| format!("function {ns}:loop_{}_poll", l.safe))
        .collect()
}

/// The one poll line of a loop (spec-0086 §7): the gate as the one gate
/// formatter writes it, then every body in the slab — players and content
/// bodies alike, each on its own — except an engine fixture and a player
/// watching a cutscene, moved by its own function at its own position.
pub(super) fn loop_poll_line(plan: &Plan, l: &crate::compiler::r#loop::LoopPlan) -> String {
    let ns = &plan.namespace;
    let gate = loop_decl(plan, l)
        .map(|d| gate_cond(plan, d.gate()))
        .unwrap_or_default();
    format!(
        "execute{gate} as @e[{},tag=!{CUTSCENE_TAG}] at @s run function {ns}:loop_{}",
        entity_box_selector(l.slab.0, l.slab.1),
        l.safe
    )
}

/// Generate a loop's functions (spec-0086 §7), in the shape of `lethal_<id>`.
///
/// * `loop_<id>_poll` — the gated selection, called from `tick`. A function of
///   its own, as `lethal_<id>` is, so the PackTest pair drives the very line the
///   tick runs, synchronously, on its own dummy.
/// * `loop_<id>` — run as and at each selected body: the count first (so an
///   `on_cross` guard reads the crossing it belongs to), then the move as
///   vanilla's all-relative `tp @s ~dx ~dy ~dz` — which is what makes every
///   component of the position packet relative (spec-0086 §2) — then the
///   dungeon's answer.
/// * `loop_<id>_cross` — the `on_cross` bundle, lowered under the root's own
///   audience ([`Audience::Scheduled`]): no command in it addresses the body,
///   exactly as a trap's payload addresses nobody.
pub(super) fn emit_loop_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for l in &plan.loops {
        fns.push((
            format!("loop_{}_poll", l.safe),
            lines(&[loop_poll_line(plan, l)]),
        ));
        let mut body: Vec<String> = Vec::new();
        if let Some(c) = &l.counts {
            body.push(format!(
                "scoreboard players add {} {} 1",
                plan::PARTY,
                plan::state_score(c)
            ));
        }
        let [dx, dy, dz] = l.offset;
        body.push(format!("tp @s ~{dx} ~{dy} ~{dz}"));
        if !l.on_cross.is_empty() {
            body.push(format!("function {ns}:loop_{}_cross", l.safe));
        }
        fns.push((format!("loop_{}", l.safe), lines(&body)));
        if !l.on_cross.is_empty() {
            fns.push((
                format!("loop_{}_cross", l.safe),
                lines(&emit_effect_bundle(
                    plan,
                    &l.on_cross,
                    root_audience(delvewright_dsl::EffectRootKind::LoopCross),
                )),
            ));
        }
    }
    fns
}
