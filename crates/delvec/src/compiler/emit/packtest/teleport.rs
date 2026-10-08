use super::*;

/// spec-0032 / `DW0545` PackTests: one template per (`teleport`, `stake`) pair,
/// each of which **leaves a real recovery-stake marker in a real teleport's
/// volume, rides, and asserts the marker stayed while a body left.**
///
/// **This is the only tier that can witness the motivating defect at all, and it
/// is worth being explicit about why.** A stake marker's position is chosen at
/// RUNTIME — the death point, or a row of the compile-time placement table picked
/// by the seat in force — so no compile-time geometry test knows where it will
/// be. That is exactly why `DW0526` (footing) and `DW0542` (an affordance bound
/// to a compile-time cell) both correctly decline it, and why the compile-time
/// half of this fix is a *class* rather than a *box test*. The compile-time proof
/// says the compiler wrote the exclusion and the marker declares the class; only
/// a live server can say vanilla's `tag=!…` really keeps that entity out of a
/// `tp`'s reach.
///
/// Three assertions, and the middle one is what stops the template being
/// vacuous:
///
/// 1. both halves of the marker — the `minecraft:interaction` the collector
///    right-clicks and the `item_display` the player sees — really are inside the
///    teleport's own selector box before anything moves (a template whose
///    fixtures landed outside would pass by examining nothing);
/// 2. a plain **body** summoned in the same box **left**, so a teleport that did
///    nothing at all cannot pass — the one-directional-falsifiability trap, where
///    a gate can only fail in the direction that never happens;
/// 3. both halves of the marker are still there.
///
/// It drives the campaign's REAL `stk_fill_<s>` and REAL `teleport_<key>`, never
/// commands it re-typed, so an emission that grows a filter or drops the class
/// reds here.
pub(super) fn emit_fixture_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    // Every stake that can leave a marker, driven by ONE template per teleport.
    //
    // The marker is a place, so every stake summons the same two entities through
    // the same `stk_place` at the same position — one template per (teleport,
    // stake) would be four templates racing to put one entity at one absolute
    // coordinate on a shared batch server, which is a collision rather than four
    // proofs. Every member of the family is still driven, in one breath, which is
    // both what `DW0810` demands and a stronger claim than the split templates
    // made: the marker that survives the ride is one a death left several wagers
    // at.
    let marking: Vec<(&delvewright_dsl::Stake, String)> = stakes(plan)
        .into_iter()
        .filter(|(st, _)| st.max_live() > 0)
        .collect();
    if marking.is_empty() {
        return;
    }
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        let Some((from, _to)) = eff.teleport() else {
            continue;
        };
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some((lo, hi)) = plan.zone_box(from) else {
            continue;
        };
        let key = &name["teleport_".len()..];
        let bx = box_selector_args(lo, hi);
        let mid = [
            (lo[0] + hi[0]) / 2,
            (lo[1] + hi[1]) / 2,
            (lo[2] + hi[2]) / 2,
        ];
        let at = format!("{} {} {}", mid[0] as f64 + 0.5, mid[1], mid[2] as f64 + 0.5);
        let tag = stk_tag();
        let hw = crate::compiler::affordance::hardware_tag(&tag);
        let body = format!("dw_fixbody_{key}");
        let (pin, me) = pin_dummy(&format!("dw_fixtest_{key}"));
        let sc = key.to_string();
        let mut t = packtest_header(&format!(
            "{title}: `{name}` carries a body out of its volume and leaves the recovery stake's \
             marker standing — a marker is a PLACE, and moving it would move the position its \
             ledger recorded (DW0545)"
        ));
        t.push(format!("function {ns}:setup"));
        t.push(pin);
        // Own entity and ledger state: a sibling template's leftovers would
        // defeat the guarded summon inside `stk_place`. Scoped to the place this
        // template is about — the marker tag is one class for the whole campaign,
        // so an unqualified `kill` here would reach into a sibling's structure.
        t.push(format!(
            "execute positioned {at} run kill @e[tag={tag},distance=..1]"
        ));
        t.push(format!(
            "execute positioned {at} run kill @e[tag={hw},distance=..1]"
        ));
        t.push(format!("kill @e[tag={body}]"));
        for (st, safe) in &marking {
            for k in 0..st.max_live() {
                t.push(format!(
                    "scoreboard players set {me} {} 0",
                    stk_live_obj(safe, k)
                ));
            }
        }
        // A real marker, put down by the real drop path, in the car — with a
        // wager on it from EVERY stake that can leave one, because that is what a
        // death which forfeits several datums leaves there.
        for (_, safe) in &marking {
            t.push(format!(
                "execute as {me} positioned {at} run function {ns}:stk_fill_{safe}"
            ));
        }
        // Bound, not assumed: both halves are inside the volume the `tp` sweeps.
        t.push(format!(
            "execute store result score #fx_in_{sc} dw.sys if entity @e[tag={tag},{bx}]"
        ));
        t.push(format!("assert score #fx_in_{sc} dw.sys matches 1"));
        t.push(format!(
            "execute store result score #fx_hw_{sc} dw.sys if entity @e[tag={hw},{bx}]"
        ));
        t.push(format!("assert score #fx_hw_{sc} dw.sys matches 1"));
        // A passenger, so "nothing moved" cannot read as a pass.
        t.push(format!(
            "summon minecraft:zombie {at} {{Tags:[\"{body}\"],NoAI:1b,Silent:1b,\
             PersistenceRequired:1b}}"
        ));
        t.push(format!("function {ns}:{name}"));
        t.push(format!(
            "execute store result score #fx_body_{sc} dw.sys if entity @e[tag={body},{bx}]"
        ));
        t.push(format!("assert score #fx_body_{sc} dw.sys matches 0"));
        // …and the place stayed a place.
        t.push(format!(
            "execute store result score #fx_stay_{sc} dw.sys if entity @e[tag={tag},{bx}]"
        ));
        t.push(format!("assert score #fx_stay_{sc} dw.sys matches 1"));
        t.push(format!(
            "execute store result score #fx_hwstay_{sc} dw.sys if entity @e[tag={hw},{bx}]"
        ));
        t.push(format!("assert score #fx_hwstay_{sc} dw.sys matches 1"));
        t.push(format!("kill @e[tag={body}]"));
        t.push(format!("kill @e[tag={tag},{bx}]"));
        t.push(format!("kill @e[tag={hw},{bx}]"));
        for (st, safe) in &marking {
            for k in 0..st.max_live() {
                t.push(format!(
                    "scoreboard players set {me} {} 0",
                    stk_live_obj(safe, k)
                ));
            }
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/fixture_{sc}.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
}

/// The entity types a `teleport` template puts in the volume — deliberately
/// **the engine's own machinery beside a content body**.
///
/// This list is the acceptance criterion made runtime-visible. `lethal_volumes[]`
/// must exempt `interaction`, `marker` and the three display types by name
/// ([`LETHAL_EXEMPT_TYPES`]) or it would erase a cutscene camera; a `teleport`
/// exempts nothing, and the four machinery types here are exactly the ones an
/// exemption list would have dropped. If a future selector grows a `type=!…`
/// term, the entity of that type stays behind and this template reds — which is
/// the point: an NPC is a body plus a co-located `minecraft:interaction`, and a
/// verb that moves one without the other loses the delve its speaker in silence.
///
/// `(type, extra NBT)`. Every one is `Silent`/`NoAI`/persistent where the type
/// supports it, so a template can never leave a wandering body behind on the
/// shared batch server.
const TELEPORT_WITNESS_TYPES: [(&str, &str); 5] = [
    // a content body — the cargo-lift ruling: everyone on the car travels
    (
        "minecraft:zombie",
        "NoAI:1b,Silent:1b,PersistenceRequired:1b",
    ),
    // the four an exemption list would have dropped
    ("minecraft:interaction", "width:1.0f,height:2.0f"),
    ("minecraft:marker", ""),
    ("minecraft:text_display", ""),
    ("minecraft:item", r#"Item:{id:"minecraft:stone",count:1}"#),
];

/// spec-0031 PackTests: one template per resolved `teleport`, each of which
/// **puts one entity of every witness type in the volume and asserts every one of
/// them arrived**.
///
/// The compile-time test (`crates/delvec/tests/v10_teleport.rs`) proves the
/// compiler wrote no filter beyond the one class exclusion (`tag=!dw_fixture`,
/// [`crate::compiler::affordance`]). That is only half of "the selection is total over
/// bodies": the other half is vanilla's own `@e[<box>]` semantics, which no Rust
/// test can witness. This template is that half, and it calls the campaign's
/// REAL generated `teleport_<key>` function — not a command it re-typed — so a
/// selector that grows a second filter reds here. Its witnesses carry no class
/// tag, which is why "the box is then empty" is still the criterion: the
/// exclusion is about the engine's own places, and this template puts none down.
///
/// The assertion is a count, not a per-entity check: the witnesses go in tagged,
/// the box is asserted to hold all of them first (a template whose entities
/// landed outside would pass by examining nothing), the function runs, and the
/// box must then hold **zero**. Counting rather than naming keeps the claim
/// exactly "every entity in the volume left it", which is the criterion's
/// wording.
pub(super) fn emit_teleport_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for eff in all_campaign_effects(plan.campaign) {
        let Some((from, _to)) = eff.teleport() else {
            continue;
        };
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some((lo, hi)) = plan.zone_box(from) else {
            continue;
        };
        let mid = [
            (lo[0] + hi[0]) / 2,
            (lo[1] + hi[1]) / 2,
            (lo[2] + hi[2]) / 2,
        ];
        let bx = box_selector_args(lo, hi);
        let key = &name["teleport_".len()..];
        let tag = format!("dw_tptest_{key}");
        let n = TELEPORT_WITNESS_TYPES.len();
        let mut t = packtest_header(&format!(
            "{title}: `{name}` moves EVERYTHING in its volume — no type is exempt (spec-0031)"
        ));
        t.push(format!("function {ns}:setup"));
        // Never assume a fresh world on the shared-batch server.
        t.push(format!("kill @e[tag={tag}]"));
        for (ty, nbt) in TELEPORT_WITNESS_TYPES {
            let sep = if nbt.is_empty() { "" } else { "," };
            t.push(format!(
                "summon {ty} {} {} {} {{Tags:[\"{tag}\"]{sep}{nbt}}}",
                mid[0] as f64 + 0.5,
                mid[1],
                mid[2] as f64 + 0.5
            ));
        }
        // Bound, not assumed: all N witnesses really are inside the volume's own
        // selector box before anything moves.
        t.push(format!(
            "execute store result score #tp_in_{key} dw.sys if entity @e[tag={tag},{bx}]"
        ));
        t.push(format!("assert score #tp_in_{key} dw.sys matches {n}"));
        t.push(format!("function {ns}:{name}"));
        // …and none of them is left behind. A `type=!…` term in the selector
        // leaves its entity here and this count is non-zero.
        t.push(format!(
            "execute store result score #tp_left_{key} dw.sys if entity @e[tag={tag},{bx}]"
        ));
        t.push(format!("assert score #tp_left_{key} dw.sys matches 0"));
        t.push(format!("kill @e[tag={tag}]"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
}
