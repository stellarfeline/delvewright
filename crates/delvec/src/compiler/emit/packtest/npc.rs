use super::*;

/// Every declared NPC's own right-click reward really re-arms that NPC.
///
/// `talk_<npc>` is per-NPC code: it revokes THAT NPC's interaction advancement,
/// runs THAT NPC's cast selector, and dispatches to a clause table whose size is
/// THAT NPC's — one clause for the gallery's warden, four for its marshal, and a
/// wholly different mechanism for its curator (a gated `show_` chooser rather
/// than a bare `dialog show`). The suite drove one of four through
/// `cast_none_silent`, whose subject is a SCENE KIND — "a `none` scene consumes
/// the interaction and opens nothing" — and is therefore a property of one
/// clause, not of any NPC.
///
/// The assertion is deliberately the one that cannot pass vacuously. On 1.21.11 a
/// function with a single invalid line is refused **in its entirety**, so a
/// `talk_` body naming a dialog id, a bark function or an advancement that does
/// not resolve runs NONE of its lines — and the reads below are exactly the ones
/// the body's own opening lines produce. A green here is the body loading and
/// running on the real server; nothing weaker is asserted, and in particular the
/// cast clause INDEX is not, because `cast_<npc>` can only ever set a value it
/// has a clause for and an assertion on its range is true by construction.
///
/// **What a template may assume about a campaign it did not author.** `talk_<npc>`
/// has two shapes, and the emitter picks between them from the campaign's own cast
/// ledger (`cast_dispatch`): with a ledger it opens `function <ns>:cast_<npc>` and
/// dispatches per clause; with none it is the single root line it always was, and
/// no `cast_<npc>` is emitted at all. The dispatch claim therefore has a subject
/// only in the first shape — asserted unconditionally it is a claim about a line
/// this campaign's body neither has nor should have, which is how it reddened
/// hello-world (one NPC, no ledger, `dw.cast` never written).
///
/// So it is gated on `crate::compiler::cast::npc_casts` — the campaign document, read
/// through the SAME authority `cast_dispatch` keys off, so the two cannot
/// disagree about which shape this NPC is. Gating it on the emitted body instead
/// would be the sixth vacuity mode exactly: the opt-out would be supplied by the
/// very defect the claim exists to catch, since a `talk_` that lost its dispatch
/// line would also lose the assertion about it. An authored ledger is a fact no
/// emitter defect can move. The stated limit, because a silent one is worse: a
/// defect in ledger DETECTION itself moves both halves, and its surface is the
/// `cast_` family rather than this claim.
pub(super) fn emit_npc_talk_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let casts = crate::compiler::cast::npc_casts(plan.campaign);
    for npc in &plan.npcs {
        let safe = &npc.safe;
        let dispatches = casts.contains_key(&npc.npc_id);
        let (pin, sel) = pin_dummy(&format!("dw_t_tlk_{safe}"));
        let mut b = packtest_header(&format!(
            "{title}: NPC `{}`'s right-click reward runs and re-arms the interaction",
            npc.npc_id
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`): the advancement record and
        // the cast score are both per-player, and every sibling test has a dummy
        // of its own on the same server.
        b.push(pin);
        // Own init: the batch is one shared server, so "never set" is not 0 — and
        // the point of the read after the call is that the call SET it. Only for
        // a body that HAS a cast dispatch; for the single-root shape there is
        // nothing to reset and nothing to read.
        if dispatches {
            b.push(format!("scoreboard players reset {sel} dw.cast"));
        }
        b.push("# Grant the interaction advancement, exactly as a right-click".to_string());
        b.push("# does: the record is written.".to_string());
        b.push(format!(
            "execute as {sel} run advancement grant @s only {ns}:{safe}_interact"
        ));
        b.push(format!("execute as {sel} run function {ns}:talk_{safe}"));
        // 1. the record is consumed, so the advancement is re-armed: a second
        // right-click still works (no dead NPC). Vanilla has no `execute if
        // advancement`; the selector argument is the primitive for reading
        // advancement state.
        b.push(format!(
            "execute as {sel} if entity @s[advancements={{{ns}:{safe}_interact=false}}] run \
             scoreboard players set #tlk_{safe} dw.sys 1"
        ));
        b.push(format!("assert score #tlk_{safe} dw.sys matches 1"));
        // 2. the cast selector really ran — its own first line writes `dw.cast`,
        // and this template reset it above, so a score at all is the proof. This
        // is what separates "the body loaded" from "the body dispatched": a
        // `talk_` whose `function <ns>:cast_<npc>` line went missing would still
        // revoke and still pass claim 1. Emitted only for an NPC the campaign
        // gave a cast ledger — see this function's doc for why the gate is the
        // authored ledger and never the emitted body.
        if dispatches {
            b.push(format!(
                "execute as {sel} store success score #cst_{safe} dw.sys if score @s dw.cast \
                 matches -2147483648.."
            ));
            b.push(format!("assert score #cst_{safe} dw.sys matches 1"));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/npc_talk_{safe}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

pub(super) fn npc_talk_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "npc-talk",
        families: vec!["talk_".to_string()],
        declared: plan.npcs.iter().map(|n| n.safe.clone()).collect(),
    }
}

/// v0.4 PackTests (spec-0008): a prop appears only once its objective activates;
/// `despawn-npc` removes the body + interaction hitbox; `move-npc` walks to the
/// target anchor. Deterministic (no combat/advancement events).
pub(super) fn emit_v04_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    moves: &[crate::compiler::nav::MovePlan],
) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    // prop appears on activation: the first interact objective carrying a prop.
    'prop: for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            if let Objective::Interact {
                id,
                anchor,
                prop: Some(prop),
                ..
            } = o
                && let Some(pos) = plan.point(area, anchor.as_str())
            {
                let mut b = packtest_header(&format!(
                    "{}: prop `{}` appears only when its objective activates",
                    artifact_title(c),
                    prop.block
                ));
                b.push(format!("function {ns}:setup"));
                b.push(format!(
                    "setblock {} {} {} minecraft:air",
                    pos[0], pos[1], pos[2]
                ));
                b.push(format!(
                    "assert block {} {} {} minecraft:air",
                    pos[0], pos[1], pos[2]
                ));
                b.push(format!(
                    "function {ns}:activate_{}",
                    safe_obj_fn(id.as_str())
                ));
                b.push(format!(
                    "assert block {} {} {} {}",
                    pos[0], pos[1], pos[2], prop.block
                ));
                write("v04_prop", b);
                break 'prop;
            }
        }
    }

    // interact-marker lifecycle: a completed interact objective leaves
    // NO `minecraft:interaction` hitbox behind — it must not stay clickable, and a
    // leaked hitbox congests the critical-path bot. Activate the first interact
    // objective (summons the hitbox), assert it exists, complete it, assert the
    // interaction count under its tag is 0.
    'cleanup: for q in &c.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            // A prop vanilla reports the use of summons no hitbox (spec-0093
            // §6.5), so there is nothing of it to clean; the template is about
            // the hitbox-carrying kind.
            if let Objective::Interact { id, anchor, .. } = o
                && plan.point(area, anchor.as_str()).is_some()
                && crate::compiler::pressable::interact_block(o).is_none()
            {
                let tag = interact_entity_tag(id.as_str());
                let (pin, sel) = pin_dummy("dw_t_iclr");
                let mut b = packtest_header(&format!(
                    "{}: completing interact `{id}` removes its interaction hitbox",
                    artifact_title(c)
                ));
                b.push(format!("function {ns}:setup"));
                // Pin this test's own dummy (see `pin_dummy`): the completion runs
                // as it alone — an `@a`-wide completion would also complete the
                // objective on every sibling test's dummy.
                b.push(pin);
                b.push(format!(
                    "function {ns}:activate_{}",
                    safe_obj_fn(id.as_str())
                ));
                b.push(format!(
                    "execute store result score #before_iclr dw.sys if entity @e[type=minecraft:interaction,tag={tag}]"
                ));
                b.push("assert score #before_iclr dw.sys matches 1..".to_string());
                b.push(format!(
                    "execute as {sel} run function {ns}:complete_{}",
                    safe_obj_fn(id.as_str())
                ));
                b.push(format!(
                    "execute store result score #after_iclr dw.sys if entity @e[type=minecraft:interaction,tag={tag}]"
                ));
                b.push("assert score #after_iclr dw.sys matches 0".to_string());
                write("v04_interact_cleanup", b);
                break 'cleanup;
            }
        }
    }

    // despawn-npc removes body + interaction hitbox (both carry the id tag), and
    // the body leaves unseen — one template per NPC a `despawn-npc` names, since
    // each `despawn_npc_<id>` is that NPC's own function (`DW0810`).
    //
    // Every root, every depth: a campaign whose only `despawn-npc` sits in a
    // `sequence` step, a trap payload or a dialogue `on_respawn` bundle still
    // gets its template.
    let despawn_targets: BTreeSet<String> = all_campaign_effects(c)
        .into_iter()
        .filter_map(|e| e.despawn_npc())
        .map(|npc| npc.as_str().to_string())
        .collect();
    for (i, npc) in despawn_targets.iter().enumerate() {
        let watched_uuid = [
            PT_WATCHED_UUID[0],
            PT_WATCHED_UUID[1],
            PT_WATCHED_UUID[2],
            PT_WATCHED_UUID[3] + i as u32,
        ];
        let safe = plan::safe_local(npc);
        let mut b = packtest_header(&format!(
            "{}: despawn-npc `{npc}` removes body + hitbox, unseen",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        // Clear EVERY planned NPC tag, not just the target's: `setup_finish`'s
        // summons are unguarded, and on the shared-batch server the world init
        // (and any sibling test) has already run it — re-running it over live
        // NPCs would duplicate every body + hitbox (mirrors `npc_summons`).
        for npc in &plan.npcs {
            b.push(format!("kill @e[tag={}]", npc.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        // A `deferred` NPC (DSL v0.6) is deliberately absent after `setup_finish` —
        // it enters via `spawn-npc`. Fire its entrance here so the despawn path is
        // exercised against the same body+hitbox pair a scripted entrance places
        // (the presence assertion below is unchanged, and stays a real assertion).
        // No line is emitted for a non-deferred target → byte-identical output for
        // campaigns that declare no deferred NPC.
        // The guard mirrors `spawn_npc_fns` exactly (planned NPC + `deferred`), so
        // the test never calls an entrance function that was not emitted.
        if plan
            .npcs
            .iter()
            .any(|n| n.npc_id == *npc && npc_is_deferred(c, &n.npc_id))
        {
            b.push(format!("function {ns}:{}", spawn_npc_fn(npc)));
        }
        // The body the test watches: the NPC's own body summon (a mannequin or
        // a villager — never the hitbox), re-issued with a fixed UUID. A selector never matches a dying body, so the UUID is how
        // the test reads the body through its death.
        let watched = plan
            .npcs
            .iter()
            .find(|n| n.npc_id == *npc)
            .and_then(|n| {
                npc_summon_commands(c, plan, n).into_iter().find(|cmd| {
                    cmd.starts_with("summon ") && !cmd.starts_with("summon minecraft:interaction ")
                })
            })
            .and_then(|cmd| with_uuid(&cmd, watched_uuid));
        if let Some(summon) = &watched {
            b.push(format!("kill @e[tag=dw_npc,tag=dw_npc_{safe}]"));
            b.push(summon.clone());
        }
        // body + interaction hitbox both carry `dw_npc_<npc>` → two entities.
        b.push(format!(
            "execute store result score #before_ndsp dw.sys if entity @e[tag=dw_npc_{safe}]"
        ));
        b.push("assert score #before_ndsp dw.sys matches 2".to_string());
        // The verb's own function — the one every `despawn-npc` site calls.
        b.push(format!("function {ns}:{}", despawn_npc_fn(npc)));
        b.push(format!(
            "execute store result score #after_ndsp dw.sys if entity @e[tag=dw_npc_{safe}]"
        ));
        b.push("assert score #after_ndsp dw.sys matches 0".to_string());
        if watched.is_some() {
            b.extend(unseen_exit_samples(&uuid_hyphenated(watched_uuid), &safe));
        }
        write(&format!("v04_despawn_{safe}"), b);
    }

    // strike trigger on an NPC's anchor (round-4 island QA): the NPC's own
    // interaction hitbox is the entity a left-click actually reaches, so it must
    // carry the trigger's tag and its `attack` record must drive the trigger.
    // Simulating the record with `/data modify` reproduces exactly what vanilla
    // writes on a left-click — the primitive under test — without needing a bot
    // to swing. Emitted only when the collision exists.
    if let Some((trigger, npc_id, npc_tag)) = first_strike_trigger_on_npc(plan) {
        let id = plan::safe_local(trigger.id.as_str());
        let hitbox = format!("@e[type=minecraft:interaction,tag={npc_tag},limit=1]");
        let mut b = packtest_header(&format!(
            "{}: striking NPC `{npc_id}` fires trigger `{}` exactly once",
            artifact_title(c),
            trigger.id.as_str()
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        // Clear EVERY planned NPC tag before re-running `setup_finish`: its
        // summons are unguarded, and the world init (and any sibling test) has
        // already run it on the shared-batch server — duplicated hitboxes would
        // break the exact-count routing assert below (mirrors `npc_summons`).
        for n in &plan.npcs {
            b.push(format!("kill @e[tag={}]", n.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        // A `deferred` NPC (DSL v0.6) is deliberately absent after `setup_finish`
        // — a sleeping giant who only enters on cue is a natural strike target, so
        // fire its entrance here (mirrors the `v04_despawn` PackTest). No line is
        // emitted for a non-deferred target.
        if npc_is_deferred(c, &npc_id) {
            b.push(format!("function {ns}:{}", spawn_npc_fn(&npc_id)));
        }
        // The routing itself: the NPC's hitbox wears the trigger's tag, so the
        // trigger's single selector reaches it.
        b.push(format!(
            "execute store result score #route_stnp dw.sys if entity @e[type=minecraft:interaction,tag={npc_tag},tag=dw_trig_{id}]"
        ));
        b.push("assert score #route_stnp dw.sys matches 1".to_string());
        // Own the dispatch gate. This template runs the REAL `tick` and asserts
        // the trigger fired, so it depends on every `#party` term the trigger's
        // arming gate reads — and those are batch-global, written by siblings and
        // held across ticks by the campaign-playthrough template. Leaving them to
        // whatever ran last is what made this test's verdict a race.
        b.extend(packtest_gate_drive(plan, trigger.gate(), true));
        if trigger.once {
            b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
        }
        // Vanilla writes this compound when a player left-clicks an interaction
        // entity; write it by hand to stand in for the swing.
        b.push(format!(
            "data modify entity {hitbox} attack set value {{player:[I;0,0,0,0],timestamp:1L}}"
        ));
        b.push(format!(
            "execute store result score #rec_stnp dw.sys if data entity {hitbox} attack"
        ));
        b.push("assert score #rec_stnp dw.sys matches 1".to_string());
        b.push(format!("function {ns}:tick"));
        if trigger.once {
            b.push(format!("assert score #trig_{id} dw.sys matches 1"));
        }
        // Exactly once: the same tick pass consumed the record, so a second pass
        // over an untouched hitbox cannot re-fire.
        b.push(format!(
            "execute store result score #rec_stnp dw.sys if data entity {hitbox} attack"
        ));
        b.push("assert score #rec_stnp dw.sys matches 0".to_string());
        if trigger.once {
            b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
            b.push(format!("function {ns}:tick"));
            b.push(format!("assert score #trig_{id} dw.sys matches 0"));
        }
        // Separability — the property the whole `strike-npc` form rests on. One
        // `minecraft:interaction` records the two click kinds in two distinct
        // NBT fields: a left-click writes `attack`, a right-click writes
        // `interaction`. Write the RIGHT-click record on the shared hitbox and
        // tick: the left-click trigger must not fire and no `attack` record may
        // appear. That is what lets the NPC's dialogue keep the right-click
        // while this trigger takes the left-click on the very same entity.
        if trigger.once {
            b.push(format!("scoreboard players set #trig_{id} dw.sys 0"));
        }
        b.push(format!(
            "data modify entity {hitbox} interaction set value {{player:[I;0,0,0,0],timestamp:1L}}"
        ));
        b.push(format!(
            "execute store result score #rc_stnp dw.sys if data entity {hitbox} attack"
        ));
        b.push("assert score #rc_stnp dw.sys matches 0".to_string());
        b.push(format!("function {ns}:tick"));
        if trigger.once {
            b.push(format!("assert score #trig_{id} dw.sys matches 0"));
        }
        b.push(format!("data remove entity {hitbox} interaction"));
        write("v04_strike_npc", b);

        // Round-6 island QA regression: the owner attacked the giant, then could
        // never open its dialogue. Root cause was not the attack — it was a
        // second, exactly co-located interaction entity (the trigger's own
        // world-init summon), which the client's ray-pick tie-break preferred,
        // so right-clicks landed on an entity without the `dw_npc_<n>` tag and
        // the dialogue advancement never fired. The invariant that ends the
        // ambiguity — and the thing this test pins — is *one cell, one hitbox*:
        // the NPC's hitbox is the only interaction entity wearing the trigger's
        // tag, before AND after an attack record lands and is consumed, so any
        // click (left or right) can only ever reach the dialogue-bearing entity.
        let mut b = packtest_header(&format!(
            "{}: attack-then-talk — NPC `{npc_id}`'s hitbox is the only click target at its anchor",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        for n in &plan.npcs {
            b.push(format!("kill @e[tag={}]", n.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        if npc_is_deferred(c, &npc_id) {
            b.push(format!("function {ns}:{}", spawn_npc_fn(&npc_id)));
        }
        // One hitbox wears the trigger tag, and none wears it without also being
        // the NPC's — the standalone summon of the pre-fix emission trips this.
        b.push(format!(
            "execute store result score #one_stlk dw.sys if entity @e[type=minecraft:interaction,tag=dw_trig_{id}]"
        ));
        b.push("assert score #one_stlk dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #orph_stlk dw.sys if entity @e[type=minecraft:interaction,tag=dw_trig_{id},tag=!{npc_tag}]"
        ));
        b.push("assert score #orph_stlk dw.sys matches 0".to_string());
        // The owner's sequence: a left-click record lands on the shared hitbox…
        // (The record is consumed by hand rather than via `tick`: a sibling
        // template's dummy may legitimately hold this trigger's gate flag, and a
        // real tick could then fire the trigger's content effects mid-test —
        // batch templates must be interleaving-independent. Consumption itself
        // is v04_strike_npc's assertion.)
        b.push(format!(
            "data modify entity {hitbox} attack set value {{player:[I;0,0,0,0],timestamp:1L}}"
        ));
        // …and the dialogue hitbox is still the one and only click target.
        b.push(format!(
            "execute store result score #one2_stlk dw.sys if entity @e[type=minecraft:interaction,tag={npc_tag}]"
        ));
        b.push("assert score #one2_stlk dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #orph2_stlk dw.sys if entity @e[type=minecraft:interaction,tag=dw_trig_{id},tag=!{npc_tag}]"
        ));
        b.push("assert score #orph2_stlk dw.sys matches 0".to_string());
        // No residue: clear the hand-written record (the runtime consume line).
        b.push(format!(
            "execute as @e[type=minecraft:interaction,tag={npc_tag}] run data remove entity @s attack"
        ));
        write("v04_strike_talk", b);
    }

    // move-npc walks a collision-safe path that ends with the NPC at the target
    // anchor. The walk is a per-tick self-scheduling driver; to assert the
    // endpoint in a single tick, fast-forward the tick counter to the final
    // waypoint and run the driver once (the reschedule it queues is harmless in a
    // PackTest). Uses the same MovePlan the emitter drove, so the asserted target
    // is the path's real final waypoint.
    if let Some(m) = moves.first() {
        let safe = plan::safe_local(&m.npc);
        let bare = movenpc_bare(&m.npc, &m.to, &m.gate_key);
        let total = m.ticks();
        let p = m.target;
        let mut b = packtest_header(&format!(
            "{}: move-npc walks to its target anchor",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        // Clear EVERY planned NPC tag before re-running the unguarded
        // `setup_finish` (see `v04_despawn`/`npc_summons`): a duplicated walker
        // would leave a stray body behind at the start cell.
        for n in &plan.npcs {
            b.push(format!("kill @e[tag={}]", n.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        // Jump the driver to its last tick, then execute the final waypoint tp.
        b.extend(walk_claim(
            Walker::Npc,
            &bare,
            &safe,
            supersedable_walkers(moves.iter().map(|m| m.npc.as_str())).contains(m.npc.as_str()),
        ));
        b.push(format!("scoreboard players set #mt_{bare} dw.sys {total}"));
        b.push(format!("function {ns}:mv_tick_{bare}"));
        b.push(format!(
            "execute store result score #npos_nmov dw.sys if entity @e[tag=dw_npc_{safe},x={},dx=0,y={},dy=0,z={},dz=0]",
            p[0], p[1], p[2]
        ));
        b.push("assert score #npos_nmov dw.sys matches 1..".to_string());
        write("v04_move", b);
    }

    // kill-less spawn-wave (spec-0008 §4 live threat): a `spawn-wave` fired from a
    // reach/interact step — with NO `kill` objective draining that wave — still
    // spawns its mobs. Regression for the emitter bug where `wave_spawn_pos`
    // resolved a spawn position ONLY from a `kill` objective, so the `spawn_<wave>`
    // function was never emitted and the effect's `function …:spawn_<wave>` call
    // dangled (the wave silently never appeared). Picks the first such wave, spawns
    // it, and asserts exactly its mob count exists under the wave tag.
    let killed: BTreeSet<&str> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .filter_map(|o| match o {
            Objective::Kill { wave, .. } => Some(wave.as_str()),
            _ => None,
        })
        .collect();
    'killless: for q in &c.quests.content.quests {
        for (obj_id, effs) in &q.on_objective_complete {
            let from_reach_or_interact = q.objectives.iter().any(|o| {
                o.id().as_str() == obj_id.as_str()
                    && matches!(
                        o,
                        Objective::ReachAnchor { .. } | Objective::Interact { .. }
                    )
            });
            if !from_reach_or_interact {
                continue;
            }
            for e in effs {
                if let Some(wave) = e.spawn_wave()
                    && !killed.contains(wave.as_str())
                    && let Some(w) = plan::wave_of(c, wave.as_str())
                {
                    let total = plan::wave_total(w);
                    let ws = plan::safe_local(wave.as_str());
                    let mut b = packtest_header(&format!(
                        "{}: kill-less spawn-wave `{wave}` spawns its mobs",
                        artifact_title(c)
                    ));
                    b.push(format!("function {ns}:setup"));
                    // Clear the wave tag first — a sibling test (`campaign` drives
                    // every objective completion, which can fire this very
                    // spawn-wave effect) may have already spawned it, and the
                    // exact-count assert needs a known-empty tag.
                    b.push(format!("kill @e[tag={}]", plan::wave_tag(wave.as_str())));
                    // No wave is live yet; the effect's driver spawns it.
                    b.push(format!("function {ns}:spawn_{ws}"));
                    b.push(format!(
                        "execute store result score #kw_klwv dw.sys if entity @e[tag={}]",
                        plan::wave_tag(wave.as_str())
                    ));
                    b.push(format!("assert score #kw_klwv dw.sys matches {total}"));
                    write("v04_killless_wave", b);
                    break 'killless;
                }
            }
        }
    }

    // Dialogue display gating: a `completes` option is DISPLAYED iff
    // its objective is active — its quest active and the objective not yet
    // complete — mirroring the click-handler guard. The chooser's `dmask_<npc>_<node>`
    // computes the per-player availability bitmask (bit `i` = the node's i-th
    // gated option is displayable); the variant it shows is `__m<mask>`. This test
    // drives that mask for the first gated completing option and asserts *that
    // option's isolated bit* (not the whole mask — sibling options in the node can
    // share a quest-active score) is 0 before the quest activates, 1 while active,
    // and 0 again after the objective completes. If the node also has a flag-gated
    // option, a final phase sets that flag in isolation and asserts its bit flips —
    // proving the flag axis is unchanged and independent of the objective-state axis.
    // EVERY gated node, and inside it every gated option — not the first of
    // either. `dmask_<npc>_<node>` is per-node code whose bit `i` is the i-th
    // gated option's own display condition, so a mask proved over one node says
    // nothing about the next: the gallery's `curator_root` gates on an objective
    // and its `curator_what` on a FORBIDDEN flag, an axis the single-node walk
    // could not even select, because it looked only for an option that completes
    // something. Registered as a claim (`dialogue_mask_watch_claim`).
    for npc in &plan.npcs {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for probe in &npc.options {
            if !seen.insert(probe.node_id.as_str()) {
                continue;
            }
            let gated = node_gated_options(npc, &probe.node_id);
            if gated.is_empty() {
                continue;
            }
            dialogue::emit_one_dialogue_mask_packtest(plan, npc, &probe.node_id, &gated, out);
        }
    }
}

/// The UUID a generated PackTest gives a body it watches through its death.
const PT_WATCHED_UUID: [u32; 4] = [0x4457_0000, 0x756e_7365, 0x656e_0000, 0x0000_0001];

/// A UUID as the int array entity NBT stores.
fn uuid_nbt(u: [u32; 4]) -> String {
    format!(
        "[I;{},{},{},{}]",
        u[0] as i32, u[1] as i32, u[2] as i32, u[3] as i32
    )
}

/// A UUID as the hyphenated form a command's entity argument takes.
fn uuid_hyphenated(u: [u32; 4]) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:04x}{:08x}",
        u[0],
        u[1] >> 16,
        u[1] & 0xffff,
        u[2] >> 16,
        u[2] & 0xffff,
        u[3]
    )
}

/// `summon <type> <x> <y> <z> {…}` with `UUID:<u>` written first into its NBT;
/// `None` for a summon that carries no NBT compound.
fn with_uuid(summon: &str, u: [u32; 4]) -> Option<String> {
    let at = summon.find('{')?;
    Some(format!(
        "{}{{UUID:{},{}",
        &summon[..at],
        uuid_nbt(u),
        &summon[at + 1..]
    ))
}

/// The PackTest lines that watch a body leave through [`removal_lines`]'
/// [`Exit::Unseen`], read by UUID (a selector never matches a dying body). A
/// death is irreversible, so three readings cover the whole exit: the body is
/// not dying on the removal's own tick, nor [`UNSEEN_DELAY_TICKS`]` - 1` ticks
/// later (the client has been told where it went before any death is sent), and
/// on the first tick it is dying — awaited — it is at [`UNSEEN_Y`], under the
/// world. A removal that kills the body where it stands reds the first; one that
/// kills it at once under the world reds the second; a body that never dies
/// times the test out. `key` names the template's own score holders: templates
/// in one batch share `dw.sys`.
fn unseen_exit_samples(uuid: &str, key: &str) -> Vec<String> {
    let floor = UNSEEN_Y + 1;
    let y = format!("#usn_y_{key}");
    let now = format!("#usn_now_{key}");
    let dying =
        format!("execute store success score {now} dw.sys if data entity {uuid} {{Health:0.0f}}");
    vec![
        dying.clone(),
        format!("assert score {now} dw.sys matches 0"),
        format!("await delay {}t", UNSEEN_DELAY_TICKS - 1),
        dying,
        format!("assert score {now} dw.sys matches 0"),
        format!("await data entity {uuid} {{Health:0.0f}}"),
        format!("execute store result score {y} dw.sys run data get entity {uuid} Pos[1]"),
        format!("assert score {y} dw.sys matches ..{floor}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The watched body's UUID reaches the command in the form vanilla prints it
    /// and the NBT in the form vanilla stores it — the same 128 bits.
    #[test]
    fn watched_uuid_forms_agree() {
        let u = PT_WATCHED_UUID;
        assert_eq!(uuid_hyphenated(u), "44570000-756e-7365-656e-000000000001");
        assert_eq!(uuid_nbt(u), "[I;1146552320,1970172773,1701707776,1]");
        assert_eq!(
            with_uuid("summon minecraft:villager 1 2 3 {NoAI:1b}", u).as_deref(),
            Some(
                "summon minecraft:villager 1 2 3 {UUID:[I;1146552320,1970172773,1701707776,1],NoAI:1b}"
            )
        );
    }
}
