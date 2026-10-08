use super::*;

/// PackTest templates for the economy (spec-0032) — **exactly the two halves this
/// tier can genuinely witness, and no template for the half it cannot.**
///
/// What a fake player is, measured twice independently (2026-08-03 and 2026-08-09):
/// permanently undamageable, and unable to die. So this tier **cannot witness a
/// player death**, and therefore cannot prove the edge from a death to a stake
/// being placed. spec-0032's acceptance criterion 9 asks for the full loop on the
/// **bot** tier for exactly that reason, and no template is generated here that
/// would appear to cover it: a template that bound to nothing and reported green
/// is the vacuity CLAUDE.md names, and it is worse than an absence because review
/// cannot see it.
///
/// What this tier CAN witness, and does:
///
/// 1. **A purchase debits, and an unaffordable one is refused and says so.** The
///    offer handler is an ordinary function; driving it as the dummy proves the
///    gate arithmetic and the refusal path without needing a click.
/// 2. **A stake drop → collect round-trip returns the exact amount.** Both ends
///    are functions, and the amount travels through the per-player ledger, so the
///    only thing skipped is the death that would normally call `stk_drop_<s>` —
///    which is precisely the part this tier cannot have and the bot tier must.
///
/// Every template pins its own dummy on the first post-`setup` line and addresses
/// it exclusively by tag, and suffixes its `dw.sys` scratch holders, per the rules
/// on [`pin_dummy`].
pub(super) fn emit_economy_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);

    // --- 1. the shop ------------------------------------------------------
    if let Some((i, sh, _)) = shops(plan).into_iter().next()
        && let Some((j, off)) = sh
            .offers
            .iter()
            .enumerate()
            .find(|(_, o)| o.effects.iter().any(|e| e.writes_state().is_some()))
        && let Some((state, _)) = off
            .effects
            .iter()
            .find_map(|e| e.writes_state())
            .map(|(s, w)| (s.clone(), w))
    {
        let obj = plan::state_score(state.as_str());
        let (pin, me) = pin_dummy("dw_shoptest");
        let mut t = packtest_header(&format!(
            "{title}: a purchase debits the purse, and one that cannot be afforded is refused \
             (spec-0032)"
        ));
        t.push(format!("function {ns}:setup"));
        t.push(pin);
        // Afford it: the balance the offer's own gate and its effects' gates both
        // read is set high enough for every comparison in the bundle to open.
        t.push(format!("scoreboard players set {me} {obj} 100"));
        t.push(format!(
            "execute as {me} run function {ns}:shop_pick_{i}_{j}"
        ));
        t.push(format!(
            "execute store result score #sh_paid dw.sys run scoreboard players get {me} {obj}"
        ));
        t.push("assert score #sh_paid dw.sys matches ..99".to_string());
        // …and cannot: with nothing in the purse the same press must move nothing.
        t.push(format!("scoreboard players set {me} {obj} 0"));
        t.push(format!(
            "execute as {me} run function {ns}:shop_pick_{i}_{j}"
        ));
        t.push(format!(
            "execute store result score #sh_broke dw.sys run scoreboard players get {me} {obj}"
        ));
        t.push("assert score #sh_broke dw.sys matches 0".to_string());
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v10_shop_purchase.mcfunction"),
            lines(&t).into_bytes(),
        );
    }

    // --- 1b. an offer's enchanted stack arrives enchanted -----------------
    // Every offer that hands over an enchanted stack is bought once by a pinned
    // dummy with an emptied inventory, and the dummy must then hold the item
    // carrying exactly those enchantments in the component the item writes them
    // to (`enchantment_component`: an enchanted book stores them). The same
    // probe is run before the purchase and must read 0 there, so a green cannot
    // come from a stack the dummy already had.
    for (i, sh, _) in shops(plan) {
        for (j, off) in sh.offers.iter().enumerate() {
            let enchanted: Vec<(&QuestEffect, &str, &std::collections::BTreeMap<String, u32>)> =
                off.effects
                    .iter()
                    .filter_map(|e| match &e.verb {
                        Verb::GiveItem {
                            item, enchantments, ..
                        } if !enchantments.is_empty() => Some((e, item.as_str(), enchantments)),
                        _ => None,
                    })
                    .collect();
            if enchanted.is_empty() {
                continue;
            }
            let (pin, me) = pin_dummy(&format!("dw_t_ench_{i}_{j}"));
            let mut t = packtest_header(&format!(
                "{title}: shop `{}` offer {j} hands over its stacks with their enchantments",
                sh.id
            ));
            t.push(format!("function {ns}:setup"));
            t.push(pin);
            t.push(format!("clear {me}"));
            // The offer's gate and each stack's own `when`, driven open as the
            // buyer, so a player-scoped datum is written on the dummy.
            for line in packtest_gate_drive(plan, off.gate(), true) {
                t.push(format!("execute as {me} run {line}"));
            }
            for (e, _, _) in &enchanted {
                for line in packtest_gate_drive(plan, e.gate(), true) {
                    t.push(format!("execute as {me} run {line}"));
                }
            }
            let probes: Vec<(String, String)> = enchanted
                .iter()
                .enumerate()
                .map(|(k, (_, item, ench))| {
                    let body = ench
                        .iter()
                        .map(|(id, lvl)| format!("\"{id}\":{lvl}"))
                        .collect::<Vec<_>>()
                        .join(",");
                    let pred = format!(
                        "{item}[{}={{{body}}}]",
                        delvewright_dsl::enchantment_component(item)
                    );
                    (format!("#ench_{i}_{j}_{k}"), pred)
                })
                .collect();
            for (holder, pred) in &probes {
                t.push(format!("scoreboard players set {holder} dw.sys 0"));
                t.push(format!(
                    "execute as {me} if items entity @s container.* {pred} run scoreboard \
                     players set {holder} dw.sys 1"
                ));
                t.push(format!("assert score {holder} dw.sys matches 0"));
            }
            t.push(format!(
                "execute as {me} run function {ns}:shop_pick_{i}_{j}"
            ));
            for (holder, pred) in &probes {
                t.push(format!(
                    "execute as {me} if items entity @s container.* {pred} run scoreboard \
                     players set {holder} dw.sys 1"
                ));
                t.push(format!("assert score {holder} dw.sys matches 1"));
            }
            out.insert(
                format!("packtest-datapack/data/{ns}/test/shop_enchanted_stack_{i}_{j}.mcfunction"),
                lines(&t).into_bytes(),
            );
        }
    }

    // --- 2. the stake's drop → collect round trip -------------------------
    for (st, safe) in stakes(plan) {
        if st.max_live() == 0 {
            continue;
        }
        let obj = plan::state_score(st.state.as_str());
        let tag = stk_tag();
        let (pin, me) = pin_dummy(&format!("dw_stktest_{safe}"));
        let mut t = packtest_header(&format!(
            "{title}: a stake takes the declared share and gives back exactly what it took \
             (spec-0032). NOT a death test — a PackTest fake player cannot die; the death edge \
             is the bot tier's (AC9)."
        ));
        t.push(format!("function {ns}:setup"));
        t.push(pin);
        // Own entity state: a sibling template's leftover marker would defeat the
        // guarded summon inside `stk_place` and make the collect assert on air.
        // Scoped to THIS dummy's own place, because the marker tag is one class for
        // the whole campaign: an unqualified `kill` would reach into every sibling
        // template's structure on the shared batch server.
        t.push(format!(
            "execute at {me} run kill @e[tag={tag},distance=..1]"
        ));
        t.push(format!(
            "execute at {me} run kill @e[tag={},distance=..1]",
            crate::compiler::affordance::hardware_tag(&tag)
        ));
        for k in 0..st.max_live() {
            t.push(format!(
                "scoreboard players set {me} {} 0",
                stk_live_obj(&safe, k)
            ));
            t.push(format!(
                "scoreboard players set {me} {} 0",
                stk_amount_obj(&safe, k)
            ));
        }
        t.push(format!("scoreboard players set {me} {obj} 40"));
        t.push(format!("execute as {me} run function {ns}:stk_drop_{safe}"));
        // The forfeit really left the purse…
        t.push(format!(
            "execute store result score #stk_lost_{safe} dw.sys run scoreboard players get {me} {obj}"
        ));
        let after_drop: i32 = match st.forfeit() {
            delvewright_dsl::Forfeit::All => 0,
            delvewright_dsl::Forfeit::None => 40,
            delvewright_dsl::Forfeit::Fixed { amount } => 40 - amount.clamp(0, 40),
            delvewright_dsl::Forfeit::Proportion { percent } => {
                40 - (40 * percent.min(100) as i32) / 100
            }
        };
        t.push(format!(
            "assert score #stk_lost_{safe} dw.sys matches {after_drop}"
        ));
        // …a marker really stands where the drop put it…
        // Existence via a counted score rather than `assert entity`: `assert score`
        // is the one assertion form every generated template in this repo already
        // uses on the pinned toolserver, so this claim is made in a spelling that
        // is known to run rather than in one that merely reads well.
        t.push(format!(
            "execute at {me} store result score #stk_mark_{safe} dw.sys if entity @e[tag={tag},distance=..1]"
        ));
        t.push(format!("assert score #stk_mark_{safe} dw.sys matches 1"));
        // …and collecting gives back exactly what was taken, no more. Driven
        // through the REAL right-click handler — the one the one advancement on
        // the one marker tag fires — never through this stake's own half of it,
        // so a collector that stopped offering the place to every stake reds here.
        //
        // The handler collects the box the player CLICKED, read off the box
        // (`stk_pick`: its last user and when), so the press is a real click
        // first: the dummy uses the box, which records it, and then the handler
        // runs as that player — what the advancement's reward does. The dummy's
        // use does not fire the advancement (vanilla triggers it in the packet
        // handler, which a dummy's use does not go through), so the handler is
        // called here as the reward would call it; the click is what it reads.
        let press = [
            format!(
                "execute at {me} run dummy {me} use entity @e[tag={tag},distance=..1,limit=1,sort=nearest]"
            ),
            format!("execute as {me} run function {ns}:{STK_COLLECT_FN}"),
        ];
        t.extend(press.iter().cloned());
        t.push(format!(
            "execute store result score #stk_back_{safe} dw.sys run scoreboard players get {me} {obj}"
        ));
        t.push(format!("assert score #stk_back_{safe} dw.sys matches 40"));
        // A second press in the same breath is a no-op — the slot went dead as part
        // of being taken, so idempotence is structural rather than timed (AC6).
        t.extend(press);
        t.push(format!(
            "execute store result score #stk_twice_{safe} dw.sys run scoreboard players get {me} {obj}"
        ));
        t.push(format!("assert score #stk_twice_{safe} dw.sys matches 40"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v10_stake_{safe}.mcfunction"),
            lines(&t).into_bytes(),
        );
    }

    // --- 3. TWO datums, ONE place -----------------------------------------
    // The template this defect owes. A death that forfeits two datums fires two
    // `drop-stake` effects at one position; the marker is a PLACE, so what must be
    // true is that the two drops leave ONE `minecraft:interaction` there, that one
    // right-click on it returns BOTH datums, and that the place then retires. Two
    // boxes at one cell would be an exact ray-pick tie — coincident `1.0 × 2.0`
    // hitboxes the client resolves by iteration order — which is the shape
    // `DW0878` refuses for authored affordances.
    //
    // Not a death test: this tier's fake player cannot die (see above), so the two
    // drops are driven the way `on_death` drives them, as two calls in one breath
    // from one position. The death EDGE stays the bot tier's (AC9); what is proved
    // here is everything downstream of it.
    let two: Vec<(&delvewright_dsl::Stake, String)> = stakes(plan)
        .into_iter()
        .filter(|(st, _)| {
            st.max_live() > 0 && !matches!(st.forfeit(), delvewright_dsl::Forfeit::None)
        })
        .take(2)
        .collect();
    if let [(a, sa), (b, sb)] = two.as_slice()
        && a.state != b.state
    {
        let tag = stk_tag();
        let hw = crate::compiler::affordance::hardware_tag(&tag);
        let (oa, ob) = (
            plan::state_score(a.state.as_str()),
            plan::state_score(b.state.as_str()),
        );
        let (pin, me) = pin_dummy("dw_stkpair");
        let mut t = packtest_header(&format!(
            "{title}: one death's two forfeits leave ONE place, and one press gives both back \
             (spec-0032). Two coincident `minecraft:interaction` boxes would be the ray-pick \
             tie `DW0878` refuses for authored affordances. NOT a death test — a PackTest fake \
             player cannot die; the death edge is the bot tier's (AC9)."
        ));
        t.push(format!("function {ns}:setup"));
        t.push(pin);
        // Scoped to this dummy's own place: the marker tag is one class for the
        // whole campaign, so an unqualified `kill` would reach into every sibling
        // template's structure on the shared batch server.
        t.push(format!(
            "execute at {me} run kill @e[tag={tag},distance=..1]"
        ));
        t.push(format!(
            "execute at {me} run kill @e[tag={hw},distance=..1]"
        ));
        for (st, safe) in [(a, sa), (b, sb)] {
            for k in 0..st.max_live() {
                t.push(format!(
                    "scoreboard players set {me} {} 0",
                    stk_live_obj(safe, k)
                ));
                t.push(format!(
                    "scoreboard players set {me} {} 0",
                    stk_amount_obj(safe, k)
                ));
            }
        }
        t.push(format!("scoreboard players set {me} {oa} 40"));
        t.push(format!("scoreboard players set {me} {ob} 40"));
        // One position, two drops — `on_death`'s own shape.
        t.push(format!(
            "execute as {me} at {me} run function {ns}:stk_drop_{sa}"
        ));
        t.push(format!(
            "execute as {me} at {me} run function {ns}:stk_drop_{sb}"
        ));
        // ONE box, not two. This is the assertion the whole template exists for:
        // `matches 1` and not `matches 1..`, because two is the defect.
        t.push(format!(
            "execute at {me} store result score #stkpair_boxes dw.sys if entity @e[tag={tag},distance=..1]"
        ));
        t.push("assert score #stkpair_boxes dw.sys matches 1".to_string());
        t.push(format!(
            "execute at {me} store result score #stkpair_hw dw.sys if entity @e[tag={hw},distance=..1]"
        ));
        t.push("assert score #stkpair_hw dw.sys matches 1".to_string());
        // Both purses really lost something, so the press below cannot pass by
        // giving back nothing.
        t.push(format!(
            "execute store result score #stkpair_lost_a dw.sys run scoreboard players get {me} {oa}"
        ));
        t.push("assert score #stkpair_lost_a dw.sys matches ..39".to_string());
        t.push(format!(
            "execute store result score #stkpair_lost_b dw.sys run scoreboard players get {me} {ob}"
        ));
        t.push("assert score #stkpair_lost_b dw.sys matches ..39".to_string());
        // One press on the one place gives BOTH datums back: a real click on the
        // box, then the handler as that player, as above.
        t.push(format!(
            "execute at {me} run dummy {me} use entity @e[tag={tag},distance=..1,limit=1,sort=nearest]"
        ));
        t.push(format!(
            "execute as {me} run function {ns}:{STK_COLLECT_FN}"
        ));
        t.push(format!(
            "execute store result score #stkpair_back_a dw.sys run scoreboard players get {me} {oa}"
        ));
        t.push("assert score #stkpair_back_a dw.sys matches 40".to_string());
        t.push(format!(
            "execute store result score #stkpair_back_b dw.sys run scoreboard players get {me} {ob}"
        ));
        t.push("assert score #stkpair_back_b dw.sys matches 40".to_string());
        // …and the place, with no wager left at it, retires — the whole of it.
        t.push(format!(
            "execute at {me} as @e[tag={tag},distance=..1] at @s run function {ns}:{STK_GC_FN}"
        ));
        t.push(format!(
            "execute at {me} store result score #stkpair_gone dw.sys if entity @e[tag={tag},distance=..1]"
        ));
        t.push("assert score #stkpair_gone dw.sys matches 0".to_string());
        t.push(format!(
            "execute at {me} store result score #stkpair_hwgone dw.sys if entity @e[tag={hw},distance=..1]"
        ));
        t.push("assert score #stkpair_hwgone dw.sys matches 0".to_string());
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v10_stake_two_datums.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
}
