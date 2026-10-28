//! Shortcut doors and their wrong-side answer (spec-0016 §2).

use super::*;

/// `setup_finish` commands for shortcut doors (spec-0016 §2, spec-0117 §3a).
/// The unlock's block and, where it starts boxed, its box are placed with every
/// pressable's ([`press_setup`]); the gate is **physically sealed in the
/// prefab** from world-load, so its own answer's boxes go up here, over the
/// door's block. Empty for a campaign with no shortcut.
pub(super) fn shortcut_setup(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        let gate = plan::safe_local(&sc.gate_anchor);
        if super::press::gate_has_answer_box(plan, &sc.gate_anchor) {
            out.push(format!(
                "execute unless entity @e[tag=dw_gate_{gate}] run function {ns}:{}",
                super::press::gate_arm_fn(&gate)
            ));
        }
    }
    out
}

/// `DW0425`: a shortcut door whose sealed side the geometry does not name.
///
/// Build tier (exit 3), raised before any function is emitted — withhold, never
/// invent. Placing the answer on a guessed side would tell a player standing
/// exactly where the door DOES open that it cannot be opened from there, which is
/// a worse failure than the silence this feature exists to end.
pub(super) fn check_shortcut_sides(plan: &Plan) -> Result<(), BuildFailure> {
    for sc in &plan.shortcuts {
        if sc.sealed_side.is_some() {
            continue;
        }
        let (lo, hi) = sc.gate_region;
        return Err(BuildFailure::Diagnostic {
            code: crate::compiler::wrongside::DW_SHORTCUT_SIDE_UNDECIDABLE,
            message: format!(
                "shortcut `{}` needs a clickable body on the sealed side of its gate `{}`, but \
                 the compiler cannot tell which side that is. The sealed side is derived from \
                 the gate slab's thin axis and the side of it the `unlock` anchor `{}` stands on; \
                 here the gate spans {lo:?}..{hi:?} and the unlock resolves to {:?}, which either \
                 gives the region no unique thinnest axis (a cube is not a doorway) or leaves the \
                 unlock level with the doorway rather than beyond it. An answer placed on a \
                 guessed side would fire where the door DOES open. Prescription: put the `unlock` \
                 clear of the gate's own span on the axis the door is thin on — which is where a \
                 far-side bar belongs anyway — or use a gate anchor whose region is a doorway \
                 slab rather than a volume.",
                sc.id, sc.gate_anchor, sc.unlock_anchor, sc.unlock,
            ),
        });
    }
    Ok(())
}

/// Per-tick shortcut unlock detection (spec-0016 §2). Fires **once** — the
/// `#sc_<id>` sentinel is the structural expression of permanence: after the open
/// there is nothing left to fire, and no verb anywhere can put the gate back
/// (`DW0372` forbids `close-gate` on a shortcut gate). Empty without a shortcut.
pub(super) fn shortcut_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        // spec-0117: a block that reports its own press is dispatched by its
        // `sc_<id>` advancement; only a box's record is read on the tick.
        if !unlock_by_box(plan, sc) {
            continue;
        }
        let id = &sc.safe;
        out.push(format!(
            "execute unless score #sc_{id} dw.sys matches 1 if entity @e[tag=dw_sc_{id},nbt={{interaction:{{}}}}] run function {ns}:shortcut_open_{id}"
        ));
        out.push(format!(
            "execute as @e[tag=dw_sc_{id}] run data remove entity @s interaction"
        ));
    }
    out
}

/// Whether a shortcut's unlock is read off its box's record (its block reports
/// no press of its own) rather than by its block's `default_block_use`.
pub(super) fn unlock_by_box(plan: &Plan, sc: &plan::ShortcutPlan) -> bool {
    super::press::press_of_owner(
        plan,
        &crate::compiler::pressable::Owner::Shortcut { id: sc.id.clone() },
    )
    .is_some_and(|p| p.boxed_in(crate::compiler::pressable::State::Open))
}

/// The `sc_<id>` advancement reward of an unlock whose block reports its own
/// press (spec-0117): `sc_press_<id>`, as the presser — revoke, mark the
/// press acted (so a `done` answer on the same press stays quiet), then open
/// the gate if it is still shut.
pub(super) fn shortcut_press_fns(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        if unlock_by_box(plan, sc)
            || !plan
                .presses()
                .iter()
                .any(|p| p.tag == format!("dw_sc_{}", sc.safe))
        {
            continue;
        }
        let id = &sc.safe;
        out.push((
            format!("sc_press_{id}"),
            lines(&[
                format!("advancement revoke @s only {ns}:sc_{id}"),
                format!(
                    "execute unless score #sc_{id} dw.sys matches 1 run tag @s add {}",
                    super::press::acted_tag(&format!("sc_{id}"))
                ),
                format!(
                    "execute unless score #sc_{id} dw.sys matches 1 run function {ns}:shortcut_open_{id}"
                ),
            ]),
        ));
    }
    out
}

/// The `shortcut_open_<id>` functions (spec-0016 §2): latch the sentinel, clear
/// the gate region to air (the same `fill … replace <block>` an `open-gate`
/// emits), then run the `on_unlock` beat. Server-source-safe — the poll lives on
/// the tick, which has no `@s`.
pub(super) fn emit_shortcut_functions(plan: &Plan) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for sc in &plan.shortcuts {
        let id = &sc.safe;
        let (from, to) = sc.gate_region;
        let mut body = vec![
            format!("scoreboard players set #sc_{id} dw.sys 1"),
            format!(
                "fill {} {} {} {} {} {} minecraft:air replace {}",
                from[0],
                from[1],
                from[2],
                to[0],
                to[1],
                to[2],
                replace_filter(&sc.gate_block)
            ),
        ];
        // …and the door's own answer goes with the bars (spec-0117): its block
        // is air now, and a box left standing in a now-walkable doorway
        // swallows presses aimed through it — the same retirement `open-gate`
        // performs for a `close-gate` seal.
        if super::press::gate_has_answer_box(plan, &sc.gate_anchor) {
            body.push(format!(
                "kill @e[tag=dw_gate_{}]",
                plan::safe_local(&sc.gate_anchor)
            ));
        }
        // The unlock is spent: its box returns over the thrown block, unless
        // released to the player's hand.
        if let Some(p) = super::press::press_of_owner(
            plan,
            &crate::compiler::pressable::Owner::Shortcut { id: sc.id.clone() },
        ) {
            body.extend(super::press::settle_lines(plan, p));
        }
        body.extend(emit_effect_bundle(
            plan,
            &sc.on_unlock,
            root_audience(delvewright_dsl::EffectRootKind::ShortcutUnlock),
        ));
        out.push((format!("shortcut_open_{id}"), lines(&body)));
    }
    out
}
