use super::*;

/// One PackTest per environment trigger: **every** trigger's own bundle really
/// runs, and a presser's right-click answer also dispatches and re-arms.
///
/// **What this closes.** A presser trigger's two bodies — `press_<id>`, the
/// advancement's reward, and `trig_<id>`, the bundle it dispatches to — were the
/// only per-object bodies in the whole gallery that the generated suite did not
/// execute at any depth: not driven by a template, and not reached transitively
/// from one either. Every other unwatched family at least *ran*. These were
/// emitted, shipped, and never once executed by anything before a player's
/// right-click in production.
///
/// **Why every trigger and not only the pressers.** Covering the pressers alone
/// would have driven two of the `trig_` family's six bodies and left four — which
/// is precisely the `DW0810` shape, arriving from inside the repair: a family the
/// suite now claims to watch, watched in part. The rule is *the suite claims to
/// watch this mechanic, so it must watch all of it*, and a fix that converts an
/// unwatched family into a partly watched one has moved the defect rather than
/// closed it.
///
/// **Why the whole declared list and not an exemplar.** Each trigger carries its
/// own gate and its own bundle: the gallery's are a label that reads itself back,
/// a door that says it is barred from the other side, a vantage that narrates, a
/// hearth that crackles, and a bay that summons something into the rafters.
/// Proving any one proves nothing about the next. Walking the list is also what
/// lets this register a [`crate::compiler::watch::Claim`], so the coverage cannot quietly
/// stop being per-object later — the failure `DW0811` exists for.
///
/// **Each body is driven the way its own dispatch route drives it.** A party
/// bundle is polled on the tick with no executor, so it is called with none; a
/// presser bundle is dispatched by an advancement that runs AS the clicker, so it
/// is called `as` the test's dummy. Driving a party bundle as a player would be a
/// stronger context than it ever really gets, which is how a template comes to
/// pass on something production would not.
///
/// For a presser, both bodies are driven, separately, because they fail
/// differently: first the bundle direct, then — after clearing the marker again,
/// or the second assert would read what the first wrote — `press_<id>` through
/// its granted advancement. Driving only the dispatch would leave the bundle
/// proven merely transitively, which is not what "watched" means here and rightly
/// does not discharge the claim.
///
/// **This reaches a body `DW0810` structurally cannot see.** The byte-read check
/// discovers objects by matching emitted function names against ids collected
/// from the *authored* stage documents, so a press answer the compiler
/// SYNTHESIZES — `close-gate`'s sealed hint, whose id is `dw_press_seal_<gate>`
/// and appears in no authored document — is not a family member as far as that
/// reading is concerned, and its two bodies were invisible to it. The claim tier
/// keys off the emitter's own authority instead, which is exactly the case the
/// two tiers are split for.
pub(super) fn emit_env_trigger_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    // The unlocalized list is the right authority here and its doc says so: this
    // asks which triggers exist and of what kind, and never reads what they say.
    for t in plan.emitted_triggers_unlocalized() {
        let id = plan::safe_local(t.id.as_str());
        // A block-bound trigger (spec-0093 §6.5) is dispatched through its
        // `press_<id>` advancement exactly as a presser is, so its dispatch half
        // is driven the same way; the bundle half keeps its own audience.
        let presser = t.addresses_presser() || trigger_is_block_bound(plan, &t);
        let step = matches!(t.on, delvewright_dsl::TriggerOn::Step);
        let (pin, sel) = pin_dummy(&format!("dw_t_trg_{id}"));
        let mut b = packtest_header(&format!(
            "{title}: environment trigger `{}` fires its own bundle{}",
            t.id,
            match (presser, step) {
                (true, false) => ", and its press answer dispatches and re-arms",
                (true, true) => ", and its step dispatches as the player who stepped",
                (false, _) => "",
            }
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        // `#trig_<id>` is a REAL runtime score and batch-global, and an authored
        // `once` puts it in the dispatch's own guard — so it is initialized here
        // rather than assumed (`pin_dummy` rule 3: "never set" is not 0).
        b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
        if presser {
            b.push(format!("scoreboard players set #prs_{id} dw.sys 0"));
        }
        // The gate is the trigger's own, driven through the one gate helper
        // rather than a partial copy of it beside this caller.
        b.extend(packtest_gate_drive(plan, t.gate(), true));
        // 1. The BUNDLE's own body. This is the object's own code — its own
        //    effects, its own gate — and nothing else in the suite runs it.
        b.push(if t.addresses_presser() {
            format!("execute as {sel} run function {ns}:trig_{id}")
        } else {
            format!("function {ns}:trig_{id}")
        });
        b.push(format!("assert score #trig_{id} dw.sys matches 1"));
        if step {
            // 2. The step's DISPATCH reaches the bundle and latches the step, so
            //    standing on does not dispatch again: a party step on
            //    `#stp_<id>`, a presser step on the player in the cell, whom it
            //    runs as.
            b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
            if presser {
                b.push(format!("execute as {sel} run function {ns}:step_{id}"));
                b.push(format!("assert score #trig_{id} dw.sys matches 1"));
                b.push(format!(
                    "execute as {sel} if entity @s[tag={STEP_TAG}{id}] run scoreboard players \
                     set #prs_{id} dw.sys 1"
                ));
                b.push(format!("assert score #prs_{id} dw.sys matches 1"));
                b.push(format!("tag {sel} remove {STEP_TAG}{id}"));
            } else {
                b.push(format!("scoreboard players set #stp_{id} dw.sys 0"));
                b.push(format!("function {ns}:step_{id}"));
                b.push(format!("assert score #trig_{id} dw.sys matches 1"));
                b.push(format!("assert score #stp_{id} dw.sys matches 1"));
            }
        }
        if !presser || step {
            out.insert(
                format!("packtest-datapack/data/{ns}/test/env_trigger_{id}.mcfunction"),
                lines(&b).into_bytes(),
            );
            continue;
        }
        // Cleared again before the dispatch half, or that half's identical
        // assert would read the value THIS half just wrote and prove nothing.
        b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
        b.push("# Grant the interaction advancement, exactly as a right-click".to_string());
        b.push("# on the trigger's hitbox does: the record is written.".to_string());
        b.push(format!(
            "execute as {sel} run advancement grant @s only {ns}:press_{id}"
        ));
        b.push(format!("execute as {sel} run function {ns}:press_{id}"));
        // 2. The DISPATCH reached the bundle. `trig_<id>`'s first line is its own
        //    ungated marker, so this separates "the reward function loaded" from
        //    "the reward function dispatched" — a `press_` whose
        //    `function <ns>:trig_<id>` line went missing would still revoke and
        //    would still pass claim 3 below.
        b.push(format!("assert score #trig_{id} dw.sys matches 1"));
        // 3. The grant is consumed, so the object answers every press — a wall is
        //    not consumed by being asked. Vanilla has no `execute if
        //    advancement`; the selector argument is the primitive for reading it.
        b.push(format!(
            "execute as {sel} if entity @s[advancements={{{ns}:press_{id}=false}}] run \
             scoreboard players set #prs_{id} dw.sys 1"
        ));
        b.push(format!("assert score #prs_{id} dw.sys matches 1"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/env_trigger_{id}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// The environment-trigger claims (`DW0811`): `trig_` over **every** declared
/// trigger, `press_` over the presser subset that owns one.
///
/// Two claims rather than one, because the two families have different declared
/// sets and a single claim would have had to name the wider one — which would
/// then demand a `press_<id>` for every party trigger, bodies that do not exist
/// and should not. `check_claims` judges only bodies that were written, so a
/// single claim would in fact have been silent about it; the reason to split is
/// that a claim should say what it means rather than rely on a later filter.
///
/// `declared` is taken from the **authored** trigger list, through the same
/// `addresses_presser()` authority `emit_advancements` and `press_dispatch_fn`
/// key off, and never from the emitted bodies. That is the half that makes a
/// claim unfakeable by the defect it guards: a template loop that stopped at
/// `first()` would still declare every trigger, whereas a claim read off `trig_*`
/// would shrink in the same stroke as the coverage.
pub(super) fn env_trigger_watch_claims(plan: &Plan) -> Vec<crate::compiler::watch::Claim> {
    let triggers = plan.emitted_triggers_unlocalized();
    vec![
        crate::compiler::watch::Claim {
            mechanic: "env-trigger",
            families: vec!["trig_".to_string()],
            declared: triggers
                .iter()
                .map(|t| plan::safe_local(t.id.as_str()))
                .collect(),
        },
        crate::compiler::watch::Claim {
            mechanic: "press-answer",
            families: vec!["press_".to_string()],
            declared: triggers
                .iter()
                .filter(|t| t.addresses_presser() && t.on.is_click())
                .map(|t| plan::safe_local(t.id.as_str()))
                .collect(),
        },
        crate::compiler::watch::Claim {
            mechanic: "step-trigger",
            families: vec!["step_".to_string()],
            declared: triggers
                .iter()
                .filter(|t| matches!(t.on, delvewright_dsl::TriggerOn::Step))
                .map(|t| plan::safe_local(t.id.as_str()))
                .collect(),
        },
    ]
}

/// The first ordered pair of click triggers that ride ONE NPC's interaction hitbox
/// and can be told apart by flags: `(npc id, npc body tag, earlier, later)`, where
/// the *later* trigger's open-assignment provably shuts the *earlier* one.
///
/// Direction matters. The starvation bug was order-dependent — the earlier-declared
/// trigger's inline `data remove` ate the click record — so the pair worth pinning
/// is exactly "the later one must still fire while the earlier one is gated off".
/// `None` when the campaign has no such pair (nothing to test, nothing emitted).
fn first_shared_hitbox_pair<'a>(
    plan: &'a Plan,
) -> Option<(
    String,
    String,
    &'a delvewright_dsl::EnvTrigger,
    &'a delvewright_dsl::EnvTrigger,
)> {
    let c = plan.campaign;
    for n in &plan.npcs {
        let anchor = c
            .npcs
            .content
            .npcs
            .iter()
            .find(|d| d.id.as_str() == n.npc_id)
            .map(|d| d.anchor.as_str())
            .unwrap_or("");
        let riders: Vec<&delvewright_dsl::EnvTrigger> = c
            .quests
            .content
            .triggers
            .iter()
            .filter(|t| trigger_rides_npc(t, anchor, &n.npc_id))
            .collect();
        for (i, a) in riders.iter().enumerate() {
            for b in &riders[i + 1..] {
                if trigger_shut_under_open(a, b) {
                    return Some((n.npc_id.clone(), n.tag.clone(), a, b));
                }
            }
        }
    }
    None
}

/// Whether `a`'s gate is shut under the flag assignment that opens `b` — every flag
/// `b` requires set, every other flag (including everything `b` forbids) unset.
fn trigger_shut_under_open(
    a: &delvewright_dsl::EnvTrigger,
    b: &delvewright_dsl::EnvTrigger,
) -> bool {
    let set: Vec<&str> = b.requires_flags.iter().map(|f| f.as_str()).collect();
    // Shut if a required flag is not among the flags this assignment sets, or a
    // forbidden flag is.
    a.requires_flags.iter().any(|f| !set.contains(&f.as_str()))
        || a.forbids_flags.iter().any(|f| set.contains(&f.as_str()))
}

/// Generated PackTest for the round-8 island defect: **two flag-gated click triggers
/// on ONE NPC hitbox, both reachable, neither starving the other**.
///
/// The island's giant carried `wake-the-giant` (requires `flag/asleep`) and
/// `his-house` (requires `flag/sealed`, forbids `flag/asleep`) on a single
/// interaction entity. The old emission cleared the `attack` record inline, per
/// trigger, immediately after that trigger's own fire clause — so the
/// earlier-declared `wake-the-giant` consumed the click even with its gate shut and
/// `his-house` could never fire. Declaration order silently decided which of two
/// legal triggers worked.
///
/// The template drives the real hardware: it writes the `attack` compound vanilla
/// writes on a left-click, runs the real `tick`, and reads the per-trigger fire
/// sentinel `#trig_<id>` (see [`env_trigger_fns`]) to see which one actually ran.
/// Both directions are asserted — the gated-off trigger must stay silent AND its
/// sibling must fire — so the test fails both on the original starvation and on any
/// future change that lets a shut gate fire.
///
/// Emitted only for a campaign that has such a pair, so every other campaign is
/// byte-identical.
pub(super) fn emit_shared_hitbox_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let Some((npc_id, npc_tag, early, late)) = first_shared_hitbox_pair(plan) else {
        return;
    };
    let a = plan::safe_local(early.id.as_str());
    let b = plan::safe_local(late.id.as_str());
    let hitbox = format!("@e[type=minecraft:interaction,tag={npc_tag},limit=1]");
    let rec_late = trigger_record(late);
    let rec_early = trigger_record(early);

    // Every flag either trigger names, deduplicated in declaration order — the set
    // the template writes and must hand back untouched (flags are party state, and
    // the batch server is shared).
    let mut flags: Vec<&str> = Vec::new();
    for t in [early, late] {
        for f in t.requires_flags.iter().chain(t.forbids_flags.iter()) {
            if !flags.contains(&f.as_str()) {
                flags.push(f.as_str());
            }
        }
    }
    // `open` writes the assignment that opens `t`: every required flag set, every
    // other named flag cleared — and then the rest of `t`'s gate, which the flag
    // pass cannot express. The union above is what makes the SIBLING's flags
    // explicit (the point of this template is that the other trigger is shut), so
    // it stays; `packtest_gate_drive` then owns `t`'s own terms, including the
    // v0.10 numeric axis this site used to drop on the floor.
    let open = |t: &delvewright_dsl::EnvTrigger| -> Vec<String> {
        let mut v: Vec<String> = flags
            .iter()
            .map(|f| {
                let want = usize::from(t.requires_flags.iter().any(|r| r.as_str() == *f));
                format!(
                    "scoreboard players set {} {} {want}",
                    plan::PARTY,
                    plan::flag_score(f)
                )
            })
            .collect();
        v.extend(state_drive_lines(plan, t.requires_state.as_slice(), true));
        v
    };

    let mut t = packtest_header(&format!(
        "{}: `{}` and `{}` share NPC `{npc_id}`'s hitbox — each fires on its own flags",
        artifact_title(c),
        early.id.as_str(),
        late.id.as_str()
    ));
    t.push(format!("function {ns}:setup"));
    t.push("scoreboard players set #placed dw.sys 1".to_string());
    // Own init (batch contract): rebuild every NPC so exactly one hitbox exists.
    for n in &plan.npcs {
        t.push(format!("kill @e[tag={}]", n.tag));
    }
    t.push(format!("function {ns}:setup_finish"));
    if npc_is_deferred(c, &npc_id) {
        t.push(format!("function {ns}:{}", spawn_npc_fn(&npc_id)));
    }
    // Precondition: ONE interaction entity wears BOTH trigger tags. Without this the
    // rest of the template would pass vacuously on two separate hitboxes.
    t.push(format!(
        "execute store result score #shr_one dw.sys if entity @e[type=minecraft:interaction,tag=dw_trig_{a},tag=dw_trig_{b}]"
    ));
    t.push("assert score #shr_one dw.sys matches 1".to_string());

    // --- The regression. Later trigger open, earlier trigger gated shut. ---
    t.extend(open(late));
    t.push(format!("scoreboard players set #trig_{a} dw.sys 0"));
    t.push(format!("scoreboard players set #trig_{b} dw.sys 0"));
    t.push(format!(
        "data modify entity {hitbox} {rec_late} set value {{player:[I;0,0,0,0],timestamp:1L}}"
    ));
    t.extend(shielded_tick(ns));
    // The starved trigger: 0 before the fix, 1 after.
    t.push(format!("assert score #trig_{b} dw.sys matches 1"));
    // …and the gated-off one stayed silent, which is what made its consumption a bug.
    t.push(format!("assert score #trig_{a} dw.sys matches 0"));
    // Consumption is unchanged: the record is gone by the end of the same pass.
    t.push(format!(
        "execute store result score #shr_rec dw.sys if data entity {hitbox} {rec_late}"
    ));
    t.push("assert score #shr_rec dw.sys matches 0".to_string());

    // --- The mirror. Earlier trigger open: it fires, so both are reachable. ---
    // Rebuild the hitbox first — the earlier trigger's own effects may have removed
    // the NPC (the island's `wake-the-giant` despawns the giant it wakes).
    for n in &plan.npcs {
        t.push(format!("kill @e[tag={}]", n.tag));
    }
    t.push(format!("function {ns}:setup_finish"));
    if npc_is_deferred(c, &npc_id) {
        t.push(format!("function {ns}:{}", spawn_npc_fn(&npc_id)));
    }
    t.extend(open(early));
    t.push(format!("scoreboard players set #trig_{a} dw.sys 0"));
    t.push(format!("scoreboard players set #trig_{b} dw.sys 0"));
    t.push(format!(
        "data modify entity {hitbox} {rec_early} set value {{player:[I;0,0,0,0],timestamp:1L}}"
    ));
    t.extend(shielded_tick(ns));
    t.push(format!("assert score #trig_{a} dw.sys matches 1"));

    // Leave no poison: clear every flag written, drop any actor the fired triggers
    // staged, and put the NPCs back the way `setup_finish` makes them.
    for f in &flags {
        t.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            plan::flag_score(f)
        ));
    }
    for act in &c.quests.content.actors {
        t.push(format!(
            "kill @e[tag=dw_actor_{}]",
            plan::safe_local(act.id.as_str())
        ));
    }
    for n in &plan.npcs {
        t.push(format!("kill @e[tag={}]", n.tag));
    }
    t.push(format!("function {ns}:setup_finish"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/v06_shared_hitbox.mcfunction"),
        lines(&t).into_bytes(),
    );
}
