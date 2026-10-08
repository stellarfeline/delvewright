//! Shops (spec-0032).

use super::*;

/// Every declared shop, paired with its index and resolved anchor cell. A shop
/// whose anchor no placed piece provides is dropped here — already `DW0142` at
/// validation, and re-reporting it from emission would blame the wrong layer.
pub(super) fn shops<'a>(plan: &'a Plan) -> Vec<(usize, &'a delvewright_dsl::Shop, [i32; 3])> {
    plan.campaign
        .quests
        .content
        .shops
        .iter()
        .enumerate()
        .filter_map(|(i, sh)| plan.point_any(sh.anchor.as_str()).map(|p| (i, sh, p)))
        .collect()
}

/// `setup` lines declaring the economy's scoreboard objectives and constants.
///
/// Empty for a campaign that declares neither a shop nor a stake — the byte-identity
/// rule every section of `setup` follows.
pub(super) fn economy_setup(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    if !shops(plan).is_empty() {
        // The answer channel and the routing channel, exactly the pair a bonfire
        // rest uses: `/trigger` is the only command a non-op player may run, so one
        // trigger objective carries every shop's answer and a dummy says which shop
        // this player opened.
        out.push("scoreboard objectives add dw.shop trigger".to_string());
        out.push("scoreboard objectives add dw.shop_at dummy".to_string());
    }
    let sts = stakes(plan);
    if sts.is_empty() {
        return out;
    }
    // The respawn point in force is the table's key, and `#cp` is where the runtime
    // keeps it. It is otherwise only written by `set-checkpoint` / a bonfire save,
    // so before the first checkpoint it would be ABSENT — and an absent score
    // matches no `matches` range, which would silently drop every entry-seat row of
    // the table. Seeded to −1, the value the entry seat's rows are keyed on.
    out.push("scoreboard players set #cp dw.sys -1".to_string());
    out.push(format!("scoreboard players set {STK_HUNDRED} dw.sys 100"));
    for (st, safe) in &sts {
        for k in 0..st.max_live() {
            out.push(format!(
                "scoreboard objectives add {} dummy",
                stk_amount_obj(safe, k)
            ));
            out.push(format!(
                "scoreboard objectives add {} dummy",
                stk_live_obj(safe, k)
            ));
            for axis in 0..3 {
                out.push(format!(
                    "scoreboard objectives add {} dummy",
                    stk_pos_obj(safe, k, axis)
                ));
            }
        }
        if let delvewright_dsl::Forfeit::Proportion { percent } = st.forfeit() {
            out.push(format!(
                "scoreboard players set #stk_p{percent} dw.sys {percent}"
            ));
        }
    }
    out
}

/// `setup_finish` lines arming every shop's affordance: the invisible
/// `minecraft:interaction` the player right-clicks and the glowing
/// `minecraft:item_display` that says there is something here.
///
/// Armed at world init rather than at a beat, because a shop is furniture — the
/// same reasoning that arms a shortcut's unlock lever at world init. Guarded by an
/// absence test so a `/reload` cannot stack a second hitbox in one cell (`DW0422`).
pub(super) fn shop_setup(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    for (i, sh, pos) in shops(plan) {
        let v = ent_xyz(pos);
        let tag = format!("dw_shop_{i}");
        out.push(format!(
            "execute unless entity @e[tag={tag}] run summon minecraft:interaction {} {} {} {{width:1.0f,height:2.0f,response:1b,Invulnerable:1b,Tags:[{FIXTURE_NBT}\"{tag}\"]}}",
            v[0], v[1], v[2]
        ));
        let hw = crate::compiler::affordance::hardware_tag(&tag);
        out.push(format!(
            "execute unless entity @e[tag={hw}] run {}",
            affordance_hardware(
                v.clone(),
                &tag,
                sh.marker_item.as_deref().unwrap_or("minecraft:emerald")
            )
        ));
    }
    out
}

/// `tick` lines for the economy.
///
/// Two dispatches, both the shape the rest of the engine already uses:
/// a shop answer read off `dw.shop`/`dw.shop_at` exactly as a bonfire's is, and the
/// stake marker garbage-collector.
///
/// **Ordering is load-bearing.** These lines are appended AFTER the death-edge
/// dispatch, because a stake dropped by `on_death` in this same tick writes its
/// slot inside that dispatch: a collector running first would see a reference count
/// of zero and delete the marker it had just been given.
pub(super) fn economy_tick(plan: &Plan) -> Vec<String> {
    let ns = &plan.namespace;
    let mut out = Vec::new();
    for (i, sh, _) in shops(plan) {
        for (j, _) in sh.offers.iter().enumerate() {
            out.push(format!(
                "execute as @a[scores={{dw.shop={},dw.shop_at={i}}}] run function {ns}:shop_pick_{i}_{j}",
                j + 1
            ));
        }
    }
    // ONE collector over ONE marker class. `max_live: 0` is the no-death-cost
    // configuration: such a stake never places a marker, so a campaign whose every
    // stake is configured that way has no marker machinery at all — not even a
    // collector looping over an empty selector.
    if stakes(plan).iter().any(|(st, _)| st.max_live() > 0) {
        out.push(format!(
            "execute as @e[tag={}] at @s run function {ns}:{STK_GC_FN}",
            stk_tag()
        ));
    }
    out
}

/// The `execute` sub-condition matching *this player's slot `k` is live and sits at
/// the marker under discussion* (`#stk_x/y/z`). Space-prefixed, so a caller splices
/// it straight onto `execute`.
pub(super) fn slot_match(safe: &str, k: u32) -> String {
    let mut s = format!(" if score @s {} matches 1", stk_live_obj(safe, k));
    for (axis, scratch) in [STK_X, STK_Y, STK_Z].iter().enumerate() {
        s.push_str(&format!(
            " if score @s {} = {scratch} dw.sys",
            stk_pos_obj(safe, k, axis)
        ));
    }
    s
}

/// Every emitted function a shop needs (DSL v0.10, spec-0032).
///
/// `shop_open_<i>` is the bonfire opener with a different dialog: revoke the
/// advancement so the shop can be opened again, record which shop this player is
/// standing at, reset-then-enable the trigger (reset both clears a stale answer and
/// re-locks the trigger — the order is what stops a previous visit's answer firing
/// as the screen opens), and show the dialog.
///
/// `shop_pick_<i>_<j>` disarms first, then applies the offer's own gate as
/// `return fail` — the same inert-to-a-direct-`/trigger` discipline a dialogue
/// option's handler uses, so a bot chatting `/trigger dw.shop set 3` cannot buy
/// what the gate refuses — and then runs the offer's effects with the buying player
/// as `@s`.
pub(super) fn emit_shop_functions(plan: &Plan) -> Vec<(String, String)> {
    let ns = &plan.namespace;
    let mut fns: Vec<(String, String)> = Vec::new();
    for (i, sh, _) in shops(plan) {
        fns.push((
            format!("shop_open_{i}"),
            lines(&[
                format!("advancement revoke @s only {ns}:shop_{i}"),
                format!("scoreboard players set @s dw.shop_at {i}"),
                "scoreboard players reset @s dw.shop".to_string(),
                "scoreboard players enable @s dw.shop".to_string(),
                format!("dialog show @s {ns}:shop_{i}"),
            ]),
        ));
        for (j, off) in sh.offers.iter().enumerate() {
            let mut body: Vec<String> = vec!["scoreboard players reset @s dw.shop".to_string()];
            for f in &off.requires_flags {
                body.push(format!(
                    "execute unless score {} {} matches 1 run return fail",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                ));
            }
            for f in &off.forbids_flags {
                body.push(format!(
                    "execute if score {} {} matches 1 run return fail",
                    plan::PARTY,
                    plan::flag_score(f.as_str())
                ));
            }
            for clause in state_clauses(plan, &off.requires_state, true) {
                body.push(format!("execute {clause} run return fail"));
            }
            body.extend(emit_effect_bundle(
                plan,
                &off.effects,
                root_audience(delvewright_dsl::EffectRootKind::ShopOffer),
            ));
            fns.push((format!("shop_pick_{i}_{j}"), lines(&body)));
        }
    }
    fns
}
