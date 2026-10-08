use super::*;

/// spec-0016 §1 bonfire PackTests. A fake player cannot die and respawn inside a
/// plain mcfunction, so — like the spec-0012 checkpoint test — these drive the
/// REAL generated `bonfire_rest_<i>` and assert its two machine-checkable
/// contracts:
///
/// * **rest moves the checkpoint**: after the rest function runs, `storage dw:cp
///   pos` reads back the bonfire cell (the mirror every other feature consumes,
///   spec-0013's boundary return included).
/// * **rest re-seats the wave**: a `respawns_on_rest` wave that was spawned and
///   then wiped is standing again after a rest, at its authored count — and a
///   wave the party never met (seated sentinel unset) is NOT summoned by a rest,
///   which is the whole point of the sentinel.
///
/// Emits nothing for a campaign with no bonfire → byte-identical.
/// The wave census counts by TAG, proven on a live server.
///
/// The ladder's old probe counted silhouettes — every entity the client tracked,
/// anything taller than half a block — so an ambush actor standing near the fight
/// was indistinguishable from a member of it, and one alive on both sides of a
/// scripted death was reported as a survivor the re-seat had failed to remove.
/// The count lives in the datapack, where the tag lives; this
/// template is what proves the arithmetic on the pinned server rather than in a
/// unit test's imagination.
///
/// Four claims: an untagged bystander standing right there is NOT counted; a
/// branded mob is; a mob that never wore the brand is not; and a wounded mob is
/// reported wounded, from the server's own `Health` and `max_health`.
///
/// Emits nothing for a campaign with no wave → byte-identical.
/// The claim the census loop makes, judged against the shipped bytes by
/// `DW0811`. Written from the campaign's authored `waves` list and NOT from
/// whatever the loop happened to walk: a walk that stopped at `first()` would
/// still declare every wave here, which is what lets the refusal fire on the
/// defect rather than on the emitter's own bookkeeping.
///
/// `declared` is every declared wave; a wave the compiler could not place emits
/// no `wave_census_<id>` body at all, and `check_claims` judges only bodies that
/// EXIST — so an unplaceable wave is not a breach, and a placed one that the
/// loop skipped is.
pub(super) fn wave_census_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "wave-census",
        families: vec!["wave_census_".to_string()],
        declared: plan
            .campaign
            .quests
            .content
            .waves
            .iter()
            .map(|w| plan::safe_local(w.id.as_str()))
            .collect(),
    }
}

pub(super) fn emit_wave_census_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    wave_placements: &WavePlacements,
) {
    // EVERY declared wave, not the first. `wave_census_<id>` dispatches into
    // `wave_census_one_<id>`, whose wounded test is written against THAT wave's
    // species and count — so a census proved over a three-zombie muster says
    // nothing about a lane of skeletons, and the gallery drove one of two.
    // Registered as a claim (`wave_census_watch_claim`).
    for w in wave_machinery_waves(plan, wave_placements) {
        emit_one_wave_census_packtest(plan, w, out);
    }
}

fn emit_one_wave_census_packtest(plan: &Plan, w: &delvewright_dsl::Wave, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    // A wave the compiler could not place emits no `spawn_<wave>` to drive.
    if plan::wave_total(w) < 1 || w.mobs.is_empty() {
        return;
    }
    let safe = plan::safe_local(w.id.as_str());
    let tag = plan::wave_tag(w.id.as_str());
    let brand = plan::wave_brand_tag(w.id.as_str());
    let total = plan::wave_total(w);
    let species = &w.mobs[0].entity;

    let mut b = packtest_header(&format!(
        "{title}: the census counts wave `{}` by TAG — a bystander beside it is not in it",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("kill @e[tag=dw_cen_by_{safe}]"));
    // `wave_census_<id>` writes its answer into `#wcen_n`/`#wcen_b`/`#wcen_d` —
    // holder names the BODY owns, so a template cannot suffix them, and with one
    // census template per wave they would be shared scratch across siblings
    // (`crate::compiler::batchstate`, `DW0807`). So each answer is copied into this wave's
    // own holder the instant it is produced, and every assertion reads the copy.
    // The copy is inside the same atomic template as the call that produced it,
    // which is what makes it a reading of this wave's census and not of whichever
    // sibling ran last.
    let cen = |b: &mut Vec<String>, src: &str, want: &str| {
        b.push(format!(
            "scoreboard players operation #wcn_{src}_{safe} dw.sys = #wcen_{src} dw.sys"
        ));
        b.push(format!(
            "assert score #wcn_{src}_{safe} dw.sys matches {want}"
        ));
    };
    b.push(format!("function {ns}:spawn_{safe}"));
    // A BYSTANDER of the wave's own species, summoned on the wave's own anchor
    // cell: everything a silhouette probe uses to decide membership, and none of
    // what the census uses. It must not move a single count.
    b.push(format!(
        "execute at @e[tag={tag},limit=1] run summon {species} ~ ~ ~ \
         {{Tags:[\"dw_cen_by_{safe}\"],PersistenceRequired:1b}}"
    ));
    b.push(format!("function {ns}:wave_census_{safe}"));
    cen(&mut b, "n", &total.to_string());
    cen(&mut b, "b", "0");
    cen(&mut b, "d", "0");
    // Brand this life's mobs. The bystander is not one of them, and the brand
    // rides the wave tag, so it cannot reach it.
    b.push(format!("function {ns}:wave_brand_{safe}"));
    b.push(format!("function {ns}:wave_census_{safe}"));
    cen(&mut b, "b", &total.to_string());
    b.push(format!(
        "execute store result score #cen_by_{safe} dw.sys if entity @e[tag=dw_cen_by_{safe},tag={brand}]"
    ));
    b.push(format!("assert score #cen_by_{safe} dw.sys matches 0"));
    // Wound one, and the census says so — read off the server's own Health and
    // max_health, not a table and not whatever the client was sent.
    b.push(format!(
        "data modify entity @e[tag={tag},limit=1] Health set value 1.0f"
    ));
    b.push(format!("function {ns}:wave_census_{safe}"));
    cen(&mut b, "d", "1");
    // A re-summon is a NEW mob: the brand cannot survive it, which is exactly the
    // property the die-retry fidelity verdict rests on.
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("function {ns}:spawn_{safe}"));
    b.push(format!("function {ns}:wave_census_{safe}"));
    cen(&mut b, "n", &total.to_string());
    cen(&mut b, "b", "0");
    cen(&mut b, "d", "0");
    // Leave no residue for the shared batch (pin_dummy rule 4).
    b.push(format!("function {ns}:wave_unbrand_{safe}"));
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("kill @e[tag=dw_cen_by_{safe}]"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/wave_census_{safe}.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// The kill reward for one wave: `k_reward_<wave>` decrements THAT wave's own
/// countdown and re-arms THAT wave's advancement.
///
/// The gallery drove one of two, and the one it did not drive is a wave with no
/// kill objective — so `verb_kill`, which reaches a reward only through an
/// objective that completes on it, structurally could never have covered it. The
/// reward body is a per-wave mechanism in its own right, so it gets a per-wave
/// proof rather than being reached sideways through the one objective that
/// happens to use it.
pub(super) fn emit_kill_reward_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    wave_placements: &WavePlacements,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for w in wave_machinery_waves(plan, wave_placements) {
        let total = plan::wave_total(w);
        let safe = plan::safe_local(w.id.as_str());
        let tag = plan::wave_tag(w.id.as_str());
        let counter = plan::wave_counter(w.id.as_str());
        let obj = plan::WAVE_OBJECTIVE;
        let (pin, sel) = pin_dummy(&format!("dw_t_kr_{safe}"));
        let mut b = packtest_header(&format!(
            "{title}: the kill reward for wave `{}` decrements its OWN countdown and re-arms",
            w.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        // Own init: the batch is one shared server and `spawn_<wave>` is
        // unguarded, so a sibling may already have fired it. Clear, then spawn —
        // which is what sets the countdown to this wave's own total.
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{safe}"));
        b.push(format!("assert score {counter} {obj} matches {total}"));
        // Grant the advancement exactly as a kill grants it. The grant is not a
        // setup step: this advancement's reward IS `k_reward_<wave>`, so granting
        // it runs the body — which is the whole wiring under test, and the reason
        // a template that ALSO called the body by hand counted two kills for one.
        b.push(format!(
            "execute as {sel} run advancement grant @s only {ns}:k_{safe}"
        ));
        b.push(format!(
            "assert score {counter} {obj} matches {}",
            total - 1
        ));
        // …and the reward consumed the record, so the NEXT kill of this wave
        // counts too. Vanilla has no `execute if advancement`; the selector
        // argument is the primitive for reading advancement state.
        b.push(format!(
            "execute as {sel} if entity @s[advancements={{{ns}:k_{safe}=false}}] run scoreboard \
             players set #kr_{safe} dw.sys 1"
        ));
        b.push(format!("assert score #kr_{safe} dw.sys matches 1"));
        // The body driven DIRECTLY, from a pinned countdown — the per-object drive
        // the watch coverage is about, and the half a grant cannot stand in for:
        // an advancement reward is resolved by name at load, so a grant proves the
        // wiring while only a call proves this wave's own body decrements this
        // wave's own counter.
        b.push(format!("scoreboard players set {counter} {obj} {total}"));
        b.push(format!(
            "execute as {sel} run function {ns}:k_reward_{safe}"
        ));
        b.push(format!(
            "assert score {counter} {obj} matches {}",
            total - 1
        ));
        // Leave no residue for the shared batch (pin_dummy rule 4): the countdown
        // is batch-global, so it goes back to the value a fresh spawn leaves.
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("scoreboard players set {counter} {obj} {total}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/wave_kill_reward_{safe}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

pub(super) fn kill_reward_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "kill-reward",
        families: vec!["k_reward_".to_string()],
        declared: plan
            .campaign
            .quests
            .content
            .waves
            .iter()
            .map(|w| plan::safe_local(w.id.as_str()))
            .collect(),
    }
}
