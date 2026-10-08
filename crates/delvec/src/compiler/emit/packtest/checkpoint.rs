use super::*;

pub(super) fn emit_bonfire_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let i = bf.index;
    let [x, y, z] = bf.pos;

    // --- rest moves the party checkpoint ---
    let mut b = packtest_header(&format!(
        "{title}: resting at a bonfire moves the party checkpoint (spec-0016 §1)"
    ));
    b.push(format!("function {ns}:setup"));
    // Scrub the shared mirror to a value the assert cannot pass by accident, then
    // run the real rest and read it back per-axis.
    b.push("data modify storage dw:cp pos set value [0, 0, 0]".to_string());
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    for (axis, want) in [(0, x), (1, y), (2, z)] {
        b.push(format!(
            "execute store result score #bc{axis}_bfr dw.sys run data get storage dw:cp pos[{axis}]"
        ));
        b.push(format!("assert score #bc{axis}_bfr dw.sys matches {want}"));
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_bonfire_rest.mcfunction"),
        lines(&b).into_bytes(),
    );

    // --- rest re-seats a wave the party has met, and only that wave ---
    let reseat = plan.reseat_waves();
    let Some(w) = reseat.first() else {
        return;
    };
    let tag = plan::wave_tag(w.id.as_str());
    let safe = plan::safe_local(w.id.as_str());
    let seated = wave_seated_holder(w.id.as_str());
    let total = plan::wave_total(w);
    let mut b = packtest_header(&format!(
        "{title}: a bonfire rest re-seats wave `{}` — but only once met (spec-0016 §1)",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    // Entity + score residue from a sibling template is batch-global: clear both.
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("scoreboard players set {seated} dw.sys 0"));
    // Unmet wave: a rest must NOT conjure it.
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #bu_bfs dw.sys if entity @e[tag={tag}]"
    ));
    b.push("assert score #bu_bfs dw.sys matches 0".to_string());
    // Met wave: spawn it, wipe it, rest — it stands again at the authored count.
    b.push(format!("function {ns}:spawn_{safe}"));
    b.push(format!("assert score {seated} dw.sys matches 1"));
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!(
        "execute store result score #bw_bfs dw.sys if entity @e[tag={tag}]"
    ));
    b.push("assert score #bw_bfs dw.sys matches 0".to_string());
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #br_bfs dw.sys if entity @e[tag={tag}]"
    ));
    b.push(format!("assert score #br_bfs dw.sys matches {total}"));
    // --- the SURVIVOR case ---
    // A wiped wave coming back proves the count. It does not prove the thing the
    // ruling is actually about: grinding a wave down one hit per life must never
    // be a valid path, so a survivor the party chipped has to be REMOVED and
    // replaced, not topped up and not left standing. Chip one mob to a sliver,
    // brand it with a tag no re-summon can carry (`spawn_<wave>` writes the
    // authored NBT and nothing else), rest, and demand the brand is gone while the
    // wave stands at full count. Identity, not just arithmetic.
    b.push(format!(
        "data modify entity @e[tag={tag},limit=1] Health set value 1.0f"
    ));
    b.push(format!("tag @e[tag={tag},limit=1] add dw_bfchip"));
    b.push("execute store result score #bp_bfs dw.sys if entity @e[tag=dw_bfchip]".to_string());
    b.push("assert score #bp_bfs dw.sys matches 1".to_string());
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push("execute store result score #bc_bfs dw.sys if entity @e[tag=dw_bfchip]".to_string());
    b.push("assert score #bc_bfs dw.sys matches 0".to_string());
    b.push(format!(
        "execute store result score #bf_bfs dw.sys if entity @e[tag={tag}]"
    ));
    b.push(format!("assert score #bf_bfs dw.sys matches {total}"));
    // Leave no residue for the rest of the batch (pin_dummy rule 4).
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("scoreboard players set {seated} dw.sys 0"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_bonfire_reseat.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §1: **a re-seated wave comes back
/// STATIONED.**
///
/// The souls loop stands — a beaten `respawns_on_rest` wave does return on a rest
/// and on a death-respawn — but it returns to the state it was FIRST seated in,
/// never to the state the party last left it in. A lane wave re-enters its routed
/// patrol from the lane start (`Patrolling:1b` re-applied, `patrol_target` back on
/// waypoint 0, the march clock back to index 0); a non-lane wave stands at its
/// anchor under vanilla-local AI with no patrol NBT at all. Nothing re-seated may
/// pursue across the map.
///
/// The engine already satisfies this by construction — the re-seat re-enters
/// through the wave's own `spawn_<wave>`, and every piece of stationed state is
/// written there — but "by construction" is folklore until a server says so. This
/// template makes it a live claim, and it is deliberately driven from the WORST
/// state the wave can be in: dragged off its lane onto the party, released to
/// native AI by the real lane clock, its march clock run down the lane, and every
/// mob branded so a survivor cannot hide inside a correct-looking count. Then it
/// runs the REAL `bonfire_rest_<i>` and demands four things:
///
/// 1. the authored count is standing;
/// 2. **not one mob of the previous life is** — the brand is gone, so this is a
///    fresh squad, not the chased one topped up;
/// 3. every mob is back at its seating footing, within the compiler-known spread
///    of the wave's own first seated cell (this is the anti-pursuit claim: the
///    mobs were 0 blocks from the player a moment ago);
/// 4. the routed state is re-asserted (lane) or absent (non-lane).
///
/// Emits nothing without both a bonfire and a `respawns_on_rest` wave →
/// byte-identical.
pub(super) fn emit_reseat_stationed_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    wave_placements: &WavePlacements,
    lane_routes: &crate::compiler::nav::LaneRoutes,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let reseat = plan.reseat_waves();
    // Prefer a LANE wave when the campaign has one: its stationed state is the
    // richer claim (the routed half), and the plain-anchor half is a subset of it.
    let Some(w) = reseat
        .iter()
        .find(|w| lane_routes.contains_key(w.id.as_str()))
        .or_else(|| reseat.first())
        .copied()
    else {
        return;
    };
    let Some(cells) = wave_placements.get(w.id.as_str()) else {
        return;
    };
    let Some(&seat) = cells.first() else {
        return;
    };
    let total = plan::wave_total(w);
    if total < 1 {
        return;
    }
    let i = bf.index;
    let safe = plan::safe_local(w.id.as_str());
    let tag = plan::wave_tag(w.id.as_str());
    let brand = plan::wave_brand_tag(w.id.as_str());
    let lane = lane_routes.get(w.id.as_str());
    // How far the wave's own seating spreads from its first cell, rounded up: the
    // exact radius the compiler placed this wave inside, so the proximity claim is
    // as tight as the geometry allows rather than a guessed slack.
    let spread = cells
        .iter()
        .map(|c| {
            (0..3)
                .map(|k| f64::from(c[k] - seat[k]).powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .fold(0.0_f64, f64::max)
        .ceil()
        .max(1.0) as i64;
    let (pin, sel) = pin_dummy("dw_rsst");

    let mut b = packtest_header(&format!(
        "{title}: a bonfire re-seat returns wave `{}` to its STATIONED state, never to the \
         chase (spec-0016 §1)",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    // A rest re-seats EVERY met wave, so this template owns the whole re-seat
    // board: clear each one's entities and its seated sentinel first, and put
    // both back at the end (pin_dummy rule 4).
    for r in &reseat {
        b.push(format!("kill @e[tag={}]", plan::wave_tag(r.id.as_str())));
        b.push(format!(
            "scoreboard players set {} dw.sys 0",
            wave_seated_holder(r.id.as_str())
        ));
    }
    // Meet the wave.
    b.push(format!("function {ns}:spawn_{safe}"));
    b.push(format!(
        "assert score {} dw.sys matches 1",
        wave_seated_holder(w.id.as_str())
    ));
    // Now put it in the state the drowned bell's ladder actually died to: the
    // squad on top of the party, off its lane, feral.
    b.push(format!("execute at {sel} run tp @e[tag={tag}] ~ ~ ~"));
    if let Some(wps) = lane {
        b.push(format!("function {ns}:lane_tick_{safe}"));
        b.push(format!(
            "execute store result score #f_rsst dw.sys if entity @e[tag={tag},nbt={{Patrolling:0b}}]"
        ));
        b.push(format!("assert score #f_rsst dw.sys matches {total}"));
        // …and its march clock run down the lane, so a re-seat that forgot to
        // reset it would send the fresh squad at the lane's far end.
        b.push(format!(
            "scoreboard players set {} dw.sys {}",
            lane_index_holder(w.id.as_str()),
            wps.len().saturating_sub(1)
        ));
    }
    // Brand this life. `spawn_<wave>` writes the authored NBT and nothing else,
    // so no re-summon can carry the stamp: identity, not arithmetic.
    b.push(format!("function {ns}:wave_brand_{safe}"));

    // --- the re-seat, through the real rest function ---
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #n_rsst dw.sys if entity @e[tag={tag}]"
    ));
    b.push(format!("assert score #n_rsst dw.sys matches {total}"));
    b.push(format!(
        "execute store result score #b_rsst dw.sys if entity @e[tag={brand}]"
    ));
    b.push("assert score #b_rsst dw.sys matches 0".to_string());
    // Back on their footing — the anti-pursuit claim.
    let c = ent_xyz(seat);
    b.push(format!(
        "execute positioned {} {} {} store result score #d_rsst dw.sys if entity \
         @e[tag={tag},distance=..{spread}]",
        c[0], c[1], c[2]
    ));
    b.push(format!("assert score #d_rsst dw.sys matches {total}"));
    match lane {
        Some(wps) => {
            b.push(format!(
                "execute store result score #p_rsst dw.sys if entity \
                 @e[tag={tag},nbt={{Patrolling:1b}}]"
            ));
            b.push(format!("assert score #p_rsst dw.sys matches {total}"));
            b.push(format!(
                "execute store result score #t_rsst dw.sys if entity \
                 @e[tag={tag},nbt={{patrol_target:[I;{},{},{}]}}]",
                wps[0][0], wps[0][1], wps[0][2]
            ));
            b.push(format!("assert score #t_rsst dw.sys matches {total}"));
            b.push(format!(
                "assert score {} dw.sys matches 0",
                lane_index_holder(w.id.as_str())
            ));
        }
        None => {
            // Vanilla-local AI only: a non-lane wave is never routed, so patrol
            // NBT must not appear on it — not on the first summon and not on a
            // re-seat.
            b.push(format!(
                "execute store result score #p_rsst dw.sys if entity \
                 @e[tag={tag},nbt={{Patrolling:1b}}]"
            ));
            b.push("assert score #p_rsst dw.sys matches 0".to_string());
        }
    }

    b.push(format!("function {ns}:wave_unbrand_{safe}"));
    for r in &reseat {
        b.push(format!("kill @e[tag={}]", plan::wave_tag(r.id.as_str())));
        b.push(format!(
            "scoreboard players set {} dw.sys 0",
            wave_seated_holder(r.id.as_str())
        ));
    }
    b.push(format!("tag {sel} remove dw_rsst"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_reseat_stationed.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §1: **an undefeated elite is put back;
/// a defeated one stays dead.**
///
/// The bell's round-five playtest found the half of the souls loop nothing was
/// driving. `respawns_on_rest` waves came back correctly — and the barrow-warden,
/// an actor elite the party had woken, wounded and run away from, stayed exactly
/// where the chase ended, at exactly the health the chase left it. So did the
/// ambushers in the sewer and up in the rafters. A rest refreshed the scene
/// around them and not them.
///
/// The two templates here are that scenario, run on the pinned server, in the
/// order it happens: meet the fight, damage it, drag it off its ground, rest —
/// then demand
///
/// 1. it is standing again, exactly one body / the authored count;
/// 2. **not the same body**: every mob of the previous life is branded with a tag
///    no summon can carry, and the brand is gone (identity, not arithmetic — the
///    anti-chip claim);
/// 3. it is back on its own ground, not where combat left it;
/// 4. an actor is put back FREED, never re-caged — a re-caged elite would be
///    dormant scenery for the rest of the delve, because the `unleash-actor` beat
///    that woke it fires from a one-shot trigger;
/// 5. and once actually killed, a rest does NOT bring it back.
///
/// Emits nothing without a bonfire, and nothing for a campaign with no hostile
/// actor and no billed elite/boss wave → byte-identical.
pub(super) fn emit_reseat_undefeated_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let i = bf.index;
    // A rest re-seats every MET `respawns_on_rest` wave too, so both templates own
    // the whole re-seat board: clear each one's entities and its seated sentinel
    // on entry and on exit (pin_dummy rule 4).
    let board: Vec<String> = plan
        .reseat_waves()
        .iter()
        .flat_map(|r| {
            [
                format!("kill @e[tag={}]", plan::wave_tag(r.id.as_str())),
                format!(
                    "scoreboard players set {} dw.sys 0",
                    wave_seated_holder(r.id.as_str())
                ),
            ]
        })
        .collect();

    // --- the actor elite (the barrow-warden's defect) ---
    if let Some(a) = plan.reseat_actors().into_iter().find(|a| {
        plan.body_point(delvewright_dsl::BodyRef::Actor(a))
            .is_some()
    }) {
        let safe = plan::safe_local(a.id.as_str());
        let origin = ent_xyz(plan.body_point(delvewright_dsl::BodyRef::Actor(a)).unwrap());
        let (pin, sel) = pin_dummy("dw_rsua");
        let mut b = packtest_header(&format!(
            "{title}: a rest re-seats the undefeated elite `{}` at its origin, and never \
             resurrects a defeated one (spec-0016 §1)",
            a.id
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.extend(board.iter().cloned());
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push("kill @e[tag=dw_rsua_brand]".to_string());
        // Meet the fight: stage the puppet, then turn it loose exactly as the
        // campaign's own beat does.
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!("function {ns}:unleash_{safe}"));
        b.push(format!(
            "execute store result score #n_rsua dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #n_rsua dw.sys matches 1".to_string());
        b.push(format!(
            "execute store result score #q_rsua dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push("assert score #q_rsua dw.sys matches 0".to_string());
        // The fight the owner had: the elite chases the party off its ground and
        // is chipped on the way.
        b.push(format!(
            "execute at {sel} run tp @e[tag=dw_actor_{safe}] ~ ~ ~"
        ));
        b.push(format!(
            "data modify entity @e[tag=dw_actor_{safe},limit=1] Health set value 1.0f"
        ));
        b.push(format!("tag @e[tag=dw_actor_{safe}] add dw_rsua_brand"));
        // The rest, through the REAL generated rest function.
        b.push(format!("function {ns}:bonfire_rest_{i}"));
        b.push(format!(
            "execute store result score #a_rsua dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #a_rsua dw.sys matches 1".to_string());
        b.push(
            "execute store result score #b_rsua dw.sys if entity @e[tag=dw_rsua_brand]".to_string(),
        );
        b.push("assert score #b_rsua dw.sys matches 0".to_string());
        b.push(format!(
            "execute positioned {} {} {} store result score #d_rsua dw.sys if entity \
             @e[tag=dw_actor_{safe},distance=..2]",
            origin[0], origin[1], origin[2]
        ));
        b.push("assert score #d_rsua dw.sys matches 1".to_string());
        // Freed, not re-caged.
        b.push(format!(
            "execute store result score #c_rsua dw.sys if entity @e[tag=dw_pup_{safe}]"
        ));
        b.push("assert score #c_rsua dw.sys matches 0".to_string());
        // Defeated stays dead: no body, nothing to put back.
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.push(format!("function {ns}:bonfire_rest_{i}"));
        b.push(format!(
            "execute store result score #k_rsua dw.sys if entity @e[tag=dw_actor_{safe}]"
        ));
        b.push("assert score #k_rsua dw.sys matches 0".to_string());
        b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        b.extend(board.iter().cloned());
        b.push(format!("tag {sel} remove dw_rsua"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/souls_reseat_actor.mcfunction"),
            lines(&b).into_bytes(),
        );
    }

    // --- the billed elite/boss wave (the anti-chip half) ---
    let Some(w) = plan
        .undefeated_reseat_waves()
        .into_iter()
        .find(|w| plan::wave_total(w) >= 1)
    else {
        return;
    };
    let safe = plan::safe_local(w.id.as_str());
    let tag = plan::wave_tag(w.id.as_str());
    let brand = plan::wave_brand_tag(w.id.as_str());
    let total = plan::wave_total(w);
    let mut b = packtest_header(&format!(
        "{title}: a rest re-seats the undefeated boss wave `{}` whole, and never resurrects a \
         beaten one (spec-0016 §1)",
        w.id
    ));
    b.push(format!("function {ns}:setup"));
    b.extend(board.iter().cloned());
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("function {ns}:wave_unbrand_{safe}"));
    // Meet it, and grind it down to a sliver — the path the ruling forbids.
    b.push(format!("function {ns}:spawn_{safe}"));
    b.push(format!("function {ns}:wave_brand_{safe}"));
    if total > 1 {
        b.push(format!("kill @e[tag={tag},limit={}]", total - 1));
    }
    b.push(format!(
        "data modify entity @e[tag={tag},limit=1] Health set value 1.0f"
    ));
    b.push(format!(
        "execute store result score #s_rsuw dw.sys if entity @e[tag={tag}]"
    ));
    b.push("assert score #s_rsuw dw.sys matches 1".to_string());
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #n_rsuw dw.sys if entity @e[tag={tag}]"
    ));
    b.push(format!("assert score #n_rsuw dw.sys matches {total}"));
    b.push(format!(
        "execute store result score #b_rsuw dw.sys if entity @e[tag={brand}]"
    ));
    b.push("assert score #b_rsuw dw.sys matches 0".to_string());
    // Beaten stays beaten: the boss the party actually killed is not conjured back.
    b.push(format!("kill @e[tag={tag}]"));
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(format!(
        "execute store result score #k_rsuw dw.sys if entity @e[tag={tag}]"
    ));
    b.push("assert score #k_rsuw dw.sys matches 0".to_string());
    b.push(format!("function {ns}:wave_unbrand_{safe}"));
    b.push(format!("kill @e[tag={tag}]"));
    b.extend(board.iter().cloned());
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_reseat_undefeated.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// **A removal the compiler performs yields nothing.** A declared drop is what a
/// player's kill yields ([`strip_drops_line`]); the unleash that kills a cage
/// and the bonfire's re-seats kill bodies too, and vanilla `/kill` is an
/// ordinary death that rolls a guaranteed slot and a death loot table whoever
/// the killer was. A playtest found the re-seat half open: every rest dropped an
/// undefeated elite's quest key where he stood.
///
/// Every such removal is [`Exit::Unseen`]: the body is moved to [`UNSEEN_Y`]
/// down its own column and dies there when [`UNSEEN_SWEEP_FN`] runs, so its loot
/// would land under the world, never at the party's feet — a count at the
/// party binds nothing. The templates therefore judge where the body dies: each
/// body is dragged onto the party first, the removal runs, what it parked in the
/// party's column is killed in the same tick ([`unseen_sweep_under`]: a dropped
/// item below the world is discarded on its own next tick, so the count cannot
/// wait for the scheduled sweep), and no item entity may lie at [`UNSEEN_Y`] in
/// the party's column.
///
/// Two templates, one per removal, because a PackTest `assert` does not abort
/// the template and the log names only the LAST failing line: one template
/// holding both removals reported a leaking unleash as the rest's failure, or
/// not at all when the rest also leaked.
///
/// - `souls_unleash_yields_nothing`: each drop-declaring hostile actor is
///   staged, its puppet dragged onto the party and the REAL `unleash_<id>` run.
///   The control: a fresh puppet — the body the unleash removes — moved to that
///   same place and killed by a bare `kill` must yield at least one item there.
/// - `souls_reseat_yields_nothing`: every re-seated body that declares a drop —
///   each wave a rest re-seats (`respawns_on_rest` or billed-undefeated) and
///   each hostile actor, met as the unleashed twin a rest re-stands — is dragged
///   onto the party and the REAL `bonfire_rest_<i>` run. What the unleash left
///   is swept and cleared first, so the count is the rest's alone. The control:
///   each fresh body killed by a bare `kill` at that place yields there.
///
/// Each template's title states its binding: how many bodies it meets, by id.
/// Emits nothing without a bonfire and a drop-declaring re-seated body.
pub(super) fn emit_reseat_yields_nothing_packtest(
    plan: &Plan,
    out: &mut BuildOutput,
    wave_placements: &WavePlacements,
) {
    let ns = &plan.namespace;
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let i = bf.index;
    let mut waves: Vec<&delvewright_dsl::Wave> = plan.reseat_waves();
    waves.extend(plan.undefeated_reseat_waves());
    waves.retain(|w| {
        wave_declares_drops(w)
            && plan::wave_total(w) >= 1
            && wave_placements
                .get(w.id.as_str())
                .is_some_and(|c| !c.is_empty())
    });
    let actors: Vec<&delvewright_dsl::Actor> = plan
        .reseat_actors()
        .into_iter()
        .filter(|a| {
            actor_declares_drops(a)
                && plan
                    .body_point(delvewright_dsl::BodyRef::Actor(a))
                    .is_some()
        })
        .collect();
    if waves.is_empty() && actors.is_empty() {
        return;
    }
    // The loot of a `/kill` lands where the body dies, in the tick it dies;
    // every body is dragged onto the party first, so an unseen removal kills it
    // at `UNSEEN_Y` in the party's column, and this radius there is the whole
    // claim.
    let items = "@e[type=minecraft:item,distance=..3]";
    // Per template: the place under its own dummy, the count there, and the
    // clear there.
    let place = |sel: &str| {
        let low = format!("execute at {sel} positioned ~ {UNSEEN_Y} ~");
        let count = {
            let low = low.clone();
            move |score: &str| format!("{low} store result score {score} dw.sys if entity {items}")
        };
        let clear = format!("{low} run kill {items}");
        (low, count, clear)
    };
    let actor_safe: Vec<String> = actors
        .iter()
        .map(|a| plan::safe_local(a.id.as_str()))
        .collect();

    if !actors.is_empty() {
        let (pin, sel) = pin_dummy("dw_usyn");
        let (low, count, clear_items) = place(&sel);
        let sweep = unseen_sweep_under(&sel);
        let mut b = packtest_header(&format!(
            "{}: the unleash yields no declared drop where the caged body dies; only a kill \
             does — binds {} unleashed bod{}: {}",
            artifact_title(plan.campaign),
            actors.len(),
            if actors.len() == 1 { "y" } else { "ies" },
            actors
                .iter()
                .map(|a| a.id.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        for safe in &actor_safe {
            b.push(format!("kill @e[tag=dw_actor_{safe}]"));
        }
        b.push(sweep.clone());
        b.push(clear_items.clone());
        for safe in &actor_safe {
            b.push(format!("function {ns}:spawn_actor_{safe}"));
            b.push(format!(
                "execute at {sel} run tp @e[tag=dw_pup_{safe}] ~ ~ ~"
            ));
            b.push(format!("function {ns}:unleash_{safe}"));
        }
        b.push(sweep.clone());
        b.push(count("#u_usyn"));
        b.push("assert score #u_usyn dw.sys matches 0".to_string());
        // Not vacuous, body by body, at the same place: the twin is put away
        // uncounted, and a fresh puppet — the body the unleash removes —
        // yields its loot to a bare kill where the removal kills.
        for safe in &actor_safe {
            b.push(format!("{low} run tp @e[tag=dw_actor_{safe}] ~ ~ ~"));
            b.push(format!("kill @e[tag=dw_actor_{safe}]"));
            b.push(clear_items.clone());
            b.push(format!("function {ns}:spawn_actor_{safe}"));
            b.push(format!("{low} run tp @e[tag=dw_pup_{safe}] ~ ~ ~"));
            b.push(format!("kill @e[tag=dw_pup_{safe}]"));
            b.push(count("#p_usyn"));
            b.push("assert score #p_usyn dw.sys matches 1..".to_string());
            b.push(clear_items.clone());
        }
        b.push(format!("tag {sel} remove dw_usyn"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/souls_unleash_yields_nothing.mcfunction"),
            lines(&b).into_bytes(),
        );
    }

    let (pin, sel) = pin_dummy("dw_rsyn");
    let (low, count, clear_items) = place(&sel);
    let sweep = unseen_sweep_under(&sel);
    let board: Vec<String> = plan
        .reseat_waves()
        .iter()
        .flat_map(|r| {
            [
                format!("kill @e[tag={}]", plan::wave_tag(r.id.as_str())),
                format!(
                    "scoreboard players set {} dw.sys 0",
                    wave_seated_holder(r.id.as_str())
                ),
            ]
        })
        .collect();
    let mut tags: Vec<String> = waves
        .iter()
        .map(|w| plan::wave_tag(w.id.as_str()))
        .collect();
    tags.extend(actor_safe.iter().map(|safe| format!("dw_actor_{safe}")));
    let mut bound: Vec<&str> = waves.iter().map(|w| w.id.as_str()).collect();
    bound.extend(actors.iter().map(|a| a.id.as_str()));

    let mut b = packtest_header(&format!(
        "{}: a bonfire re-seat yields no declared drop where the body dies; only a kill \
         does — binds {} re-seated bod{}: {}",
        artifact_title(plan.campaign),
        bound.len(),
        if bound.len() == 1 { "y" } else { "ies" },
        bound.join(", ")
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    b.extend(board.iter().cloned());
    for t in &tags {
        let k = format!("kill @e[tag={t}]");
        if !board.contains(&k) {
            b.push(k);
        }
    }
    b.push(sweep.clone());
    b.push(clear_items.clone());
    // Meet every fight, on top of the party; a hostile actor is met as the
    // unleashed twin a rest re-stands.
    for w in &waves {
        b.push(format!(
            "function {ns}:spawn_{}",
            plan::safe_local(w.id.as_str())
        ));
    }
    for safe in &actor_safe {
        b.push(format!("function {ns}:spawn_actor_{safe}"));
        b.push(format!(
            "execute at {sel} run tp @e[tag=dw_pup_{safe}] ~ ~ ~"
        ));
        b.push(format!("function {ns}:unleash_{safe}"));
    }
    // What the unleash parked is its own template's claim: put it away
    // uncounted, so this count is the rest's alone.
    b.push(sweep.clone());
    b.push(clear_items.clone());
    for t in &tags {
        b.push(format!("execute at {sel} run tp @e[tag={t}] ~ ~ ~"));
    }
    // The rest, through the REAL generated rest function, and the removals it
    // makes, killed where the removal parked them.
    b.push(format!("function {ns}:bonfire_rest_{i}"));
    b.push(sweep.clone());
    b.push(count("#r_rsyn"));
    b.push("assert score #r_rsyn dw.sys matches 0".to_string());
    // Not vacuous, body by body, at the same place: each fresh body carries the
    // loot, and a bare kill where the removal kills yields it.
    for t in &tags {
        b.push(format!("{low} run tp @e[tag={t}] ~ ~ ~"));
        b.push(format!("kill @e[tag={t}]"));
        b.push(count("#p_rsyn"));
        b.push("assert score #p_rsyn dw.sys matches 1..".to_string());
        b.push(clear_items.clone());
    }
    b.extend(board.iter().cloned());
    b.push(format!("tag {sel} remove dw_rsyn"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_reseat_yields_nothing.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §1: the **two options really differ**.
///
/// Right-clicking a bonfire offers exactly *rest and save* and *save only*, and
/// save-only does nothing but move the checkpoint. That is a runtime claim about
/// two functions, and this drives both on a live server through the flask — the one restored resource a PackTest
/// dummy can actually observe.
///
/// **Why the flask and not health.** PackTest fake players are immune to
/// `/damage` (measured on the pinned toolserver, 2026-08-03: `Health` stays at
/// 20.0 through `damage @s 1000`), so a dummy can never be *hurt* and therefore
/// never be seen to be *healed* — an assertion on health would be permanently
/// red no matter how correct the engine is. Inventory has no such problem:
/// `clear <player> <item> 0` counts matching items without removing them, so the
/// template can spend the flask down to one, drive each option, and read the
/// count back. The heal/feed/cure half of a rest is proven where it can be proven
/// honestly — compiler unit tests assert the exact `effect` commands and their
/// order inside `bonfire_restore`.
///
/// Emits nothing for a campaign without both a bonfire and a flask, which
/// `DW0476` makes the same thing as "no bonfire" → byte-identical.
pub(super) fn emit_bonfire_option_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let Some(&(ci, ki)) = plan.flasks().first() else {
        return;
    };
    let i = bf.index;
    let item = &plan.campaign.classes.content.classes[ci].kit[ki];
    let ctag = class_tag(&plan.classes[ci].safe);
    let (pin, sel) = pin_dummy("dw_bfopt");
    // Counting predicate: the flask's own item predicate, so on a
    // contents-bearing flask every count below is of bottles whose
    // `potion_contents` matches EXACTLY. That is what makes this template a
    // proof of round-trip and not merely of arithmetic — a rest that re-gave a
    // differently-filled bottle (or the contents-less placeholder) would leave
    // the exact-match count at 1 while the bare-id count climbed to `count + 1`,
    // and both halves are asserted below.
    let pred = kit_item_predicate(item);
    let comp = kit_item_components(item);

    let mut b = packtest_header(&format!(
        "{title}: save-only saves and nothing else; rest refills the flask (spec-0016 §1)"
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    // The dummy takes the flask's class, so `bonfire_flask`'s per-class guard
    // selects it — this is the same tag `class_apply_<class>` adds.
    b.push(format!("tag {sel} add {ctag}"));
    // Baseline: exactly ONE flask in the bag (the party has spent the rest),
    // filled exactly as the class kit fills it.
    b.push(format!("clear {sel} {}", item.item));
    b.push(format!("give {sel} {}{comp} 1", item.item));

    // --- save only: the checkpoint moves, the flask does NOT come back ---
    b.push("data modify storage dw:cp pos set value [0, 0, 0]".to_string());
    b.push(format!(
        "execute as {sel} run function {ns}:bonfire_pick_save_{i}"
    ));
    b.push(format!(
        "execute store result score #bo_save dw.sys run clear {sel} {pred} 0"
    ));
    b.push("assert score #bo_save dw.sys matches 1".to_string());
    b.push(
        "execute store result score #bo_cp dw.sys run data get storage dw:cp pos[0]".to_string(),
    );
    b.push(format!("assert score #bo_cp dw.sys matches {}", bf.pos[0]));

    // --- rest and save: the flask is replenished to its declared count ---
    b.push(format!(
        "execute as {sel} run function {ns}:bonfire_pick_rest_{i}"
    ));
    b.push(format!(
        "execute store result score #bo_rest dw.sys run clear {sel} {pred} 0"
    ));
    b.push(format!(
        "assert score #bo_rest dw.sys matches {}",
        item.count
    ));
    // …and nothing ELSE of that item id is in the bag: refilling by handing over
    // a second, differently-filled bottle is the failure this catches.
    b.push(format!(
        "execute store result score #bo_any dw.sys run clear {sel} {} 0",
        item.item
    ));
    b.push(format!(
        "assert score #bo_any dw.sys matches {}",
        item.count
    ));

    // Leave no residue for the shared batch (pin_dummy rule 4).
    b.push(format!("clear {sel} {}", item.item));
    b.push(format!("tag {sel} remove {ctag}"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_bonfire_options.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// spec-0016 §1: **a rest mends what the player carries and takes back the
/// flask's empties — and nothing else.**
///
/// Driven on the framework dummy (`@s`) through the real
/// `bonfire_pick_rest_<i>`. Before the rest the dummy carries a damaged shield in
/// the off-hand, a damaged chestplate in the armour slot, a damaged sword in the
/// hotbar and a damaged bow in the last inventory slot (the four mends), an
/// undamaged sword and a stack of bread (must be untouched), five glass bottles it
/// "found" (must survive), a flask as the kit gives it (whose marked remainder is
/// read back through the server's own component predicate) and two of the empties
/// that remainder is.
///
/// **Why the dummy does not drink.** Drinking takes 32 ticks, so a drink is a
/// multi-tick step in a batch that shares one server. Measured on the pinned
/// toolserver: across 42 full-suite runs of this template with a real
/// `dummy @s use item` (variants carrying diagnostics and the isolating
/// perturbations tried: no cutscene sibling, the join teleport and the class
/// dialog pre-empted), the potion was still undrunk in the hand at the read in
/// 17 — cause not found — so a drink
/// here would be an intermittent gate. That vanilla's consume hands back exactly
/// the `use_remainder` stack is cited (the pinned `item_components` report gives
/// `minecraft:potion` its bottle through that component) and was measured on the
/// same server by a probe; see `docs/reference/compiler.md`, Stage 3 `flask`.
///
/// Unlike health (PackTest dummies are immune to `/damage`, see
/// [`emit_bonfire_option_packtest`]), item durability is plain component state,
/// so the repair is observable on a dummy.
///
/// Emits nothing without a bonfire and a flask (`DW0476` makes those the same).
/// The empties half needs a flask whose item leaves a remainder; a flask that
/// leaves none has no empties, so that half is not written.
pub(super) fn emit_bonfire_mend_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let Some(bf) = plan.bonfires().next() else {
        return;
    };
    let Some(&(ci, ki)) = plan.flasks().first() else {
        return;
    };
    let i = bf.index;
    let item = &plan.campaign.classes.content.classes[ci].kit[ki];
    let ctag = class_tag(&plan.classes[ci].safe);
    let pred = kit_item_predicate(item);
    let comp = kit_item_components(item);
    let remainder = flask_remainder(item);

    let mut b = packtest_header(&format!(
        "{title}: a rest mends every carried item in place and takes back only the \
         flask's empties (spec-0016 §1)"
    ));
    b.push(format!("function {ns}:setup"));
    b.push(format!("tag @s add {ctag}"));
    b.push("clear @s".to_string());
    // Gear to mend, one per kind of carried slot.
    b.push("item replace entity @s weapon.offhand with minecraft:shield[damage=300]".to_string());
    b.push(
        "item replace entity @s armor.chest with minecraft:iron_chestplate[damage=100]".to_string(),
    );
    b.push("item replace entity @s container.1 with minecraft:iron_sword[damage=200]".to_string());
    b.push("item replace entity @s container.35 with minecraft:bow[damage=50]".to_string());
    // What the rest must not touch.
    b.push("item replace entity @s container.2 with minecraft:iron_sword".to_string());
    b.push("item replace entity @s container.3 with minecraft:bread 7".to_string());
    b.push("item replace entity @s container.4 with minecraft:glass_bottle 5".to_string());
    if let Some((id, count)) = remainder {
        // The flask as the kit hands it out carries the marked remainder — read
        // back through the server's own component predicate, so a mark the game
        // parsed differently (or dropped) fails here…
        b.push(format!(
            "item replace entity @s container.5 with {}{comp}",
            item.item
        ));
        b.push(format!(
            "execute store result score #mark_bfmd dw.sys if items entity @s container.5 \
             {}[use_remainder={}]",
            item.item,
            flask_remainder_snbt(id, count)
        ));
        b.push("assert score #mark_bfmd dw.sys matches 1".to_string());
        // …and two of the empties that remainder is, as drinking leaves them.
        b.push(format!(
            "item replace entity @s container.6 with {} 2",
            flask_empty_stack(id)
        ));
    }

    b.push(format!("function {ns}:bonfire_pick_rest_{i}"));

    // The four damaged items are whole again, in the slots they were in.
    b.push("scoreboard players set #mend_bfmd dw.sys 0".to_string());
    for (slot, it) in [
        ("weapon.offhand", "minecraft:shield"),
        ("armor.chest", "minecraft:iron_chestplate"),
        ("container.1", "minecraft:iron_sword"),
        ("container.35", "minecraft:bow"),
    ] {
        b.push(format!(
            "execute if items entity @s {slot} {it}[damage=0] run scoreboard players add \
             #mend_bfmd dw.sys 1"
        ));
    }
    b.push("assert score #mend_bfmd dw.sys matches 4".to_string());
    // Untouched: the whole sword, the stack, and nobody re-kitted the player
    // (the kit's own sword would be a second one).
    b.push("scoreboard players set #keep_bfmd dw.sys 0".to_string());
    b.push(
        "execute if items entity @s container.2 minecraft:iron_sword[damage=0] run \
         scoreboard players add #keep_bfmd dw.sys 1"
            .to_string(),
    );
    b.push(
        "execute if items entity @s container.3 minecraft:bread run scoreboard players add \
         #keep_bfmd dw.sys 1"
            .to_string(),
    );
    b.push("assert score #keep_bfmd dw.sys matches 2".to_string());
    b.push(
        "execute store result score #bread_bfmd dw.sys run clear @s minecraft:bread 0".to_string(),
    );
    b.push("assert score #bread_bfmd dw.sys matches 7".to_string());
    b.push(
        "execute store result score #sword_bfmd dw.sys run clear @s minecraft:iron_sword 0"
            .to_string(),
    );
    b.push("assert score #sword_bfmd dw.sys matches 2".to_string());
    // The flask's empties are gone; the bottles found elsewhere are not.
    if let Some((id, _)) = remainder {
        b.push(format!(
            "execute store result score #mine_bfmd dw.sys run clear @s {} 0",
            flask_empty_stack(id)
        ));
        b.push("assert score #mine_bfmd dw.sys matches 0".to_string());
    }
    b.push(
        "execute store result score #glass_bfmd dw.sys run clear @s minecraft:glass_bottle 0"
            .to_string(),
    );
    b.push("assert score #glass_bfmd dw.sys matches 5".to_string());
    // And the flask is back at its declared count.
    b.push(format!(
        "execute store result score #flask_bfmd dw.sys run clear @s {pred} 0"
    ));
    b.push(format!(
        "assert score #flask_bfmd dw.sys matches {}",
        item.count
    ));

    // Leave no residue for the shared batch (pin_dummy rule 4).
    b.push("clear @s".to_string());
    b.push(format!("tag @s remove {ctag}"));
    out.insert(
        format!("packtest-datapack/data/{ns}/test/souls_bonfire_mend.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// v0.6 PackTests (spec-0012 checkpoints, spec-0014 stealth). Fake players cannot
/// respawn synchronously within a plain mcfunction test, so these drive the
/// compiler-generated mechanics directly and assert their deterministic effects:
///
/// * **checkpoint**: applying the checkpoint's `spawnpoint @a` + `dw:cp pos`
///   mirror makes `storage dw:cp pos` read back the checkpoint cell — the
///   machine-checkable "last checkpoint" contract other features consume.
/// * **stealth** (zone-presence model, no sneak
///   requirement): the generated `stealth_eval_<i>` judge catches an exposed
///   (out-of-zone) player after `grace_ticks` and spares an in-zone one —
///   driven by teleporting the dummy in and out of the declared zone box.
pub(super) fn emit_v06_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);

    if let Some(cp) = plan.checkpoints.first() {
        let [x, y, z] = cp.pos;
        let (pin, sel) = pin_dummy("dw_t_cpr");
        let mut t = packtest_header(&format!(
            "{title}: checkpoint mirrors its cell into dw:cp (spec-0012)"
        ));
        t.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`): the spawnpoint write is
        // per-player, so it goes to this test's dummy, not every dummy in the
        // batch. The `dw:cp` mirror write + read-back stay within this single
        // (atomic) function, so the shared storage cannot be interleaved.
        t.push(pin);
        // Apply the exact commands a `set-checkpoint` emits, then read the mirror
        // back per-axis (rock-solid vs. an NBT compound match).
        t.push(format!("spawnpoint {sel} {x} {y} {z}"));
        t.push(format!(
            "data modify storage dw:cp pos set value [{x}, {y}, {z}]"
        ));
        t.push(
            "execute store result score #cx_cpr dw.sys run data get storage dw:cp pos[0]"
                .to_string(),
        );
        t.push(
            "execute store result score #cy_cpr dw.sys run data get storage dw:cp pos[1]"
                .to_string(),
        );
        t.push(
            "execute store result score #cz_cpr dw.sys run data get storage dw:cp pos[2]"
                .to_string(),
        );
        t.push(format!("assert score #cx_cpr dw.sys matches {x}"));
        t.push(format!("assert score #cy_cpr dw.sys matches {y}"));
        t.push(format!("assert score #cz_cpr dw.sys matches {z}"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_checkpoint_respawn.mcfunction"),
            lines(&t).into_bytes(),
        );

        // --- the environmental-death variant ---
        //
        // The template above proves the RECORD; this one proves the LANDING, which
        // is the half the owner's tide-mill playtest found missing. `spawnpoint` is
        // only a hint: vanilla re-validates the recorded cell on death and silently
        // respawns at the world spawn when it is solid or liquid. Nothing about
        // that is specific to how the player died — a crush gate's
        // `damage @s 1000 minecraft:generic` leaves exactly the same `deathCount`
        // edge a mob kill does — so the test drives that edge directly, from the
        // worst starting position (the campaign entrance, where vanilla's fallback
        // drops them), and asserts the player ends on the checkpoint cell.
        //
        // Second half: the re-seat must be EDGE-triggered. A leash that re-seated
        // every tick would pin the party to the checkpoint and make the delve
        // unplayable, so the test walks the dummy away again, re-runs the check
        // with no new death, and asserts it stayed away.
        if let Some(entry) = campaign_spawn(plan) {
            let (pin, sel) = pin_dummy("dw_t_cpseat");
            let mut t = packtest_header(&format!(
                "{title}: an environmental death re-seats the player ON the checkpoint, once \
                 (spec-0012)"
            ));
            t.push(format!("function {ns}:setup"));
            t.push(pin);
            t.push(format!("scoreboard players set #cp dw.sys {}", cp.index));
            t.push(format!("scoreboard players set {sel} dw.death_ack 0"));
            t.push(format!("scoreboard players set {sel} dw.deaths 1"));
            t.push(format!(
                "tp {sel} {} {} {}",
                center(entry[0]),
                entry[1],
                center(entry[2])
            ));
            // With a declared respawn wait (spec-0077) the edge first asks
            // whether this player waits, from the counts the tick took. The
            // template states the premise in which nobody waits: a party of two
            // whose other member is not in play, and this player not wiped.
            // `v06_checkpoint_wait` proves the other premise.
            if respawn_wait(plan).is_some() {
                t.push(format!("scoreboard players set {RW_PRESENT} dw.sys 2"));
                t.push(format!("scoreboard players set {ALIVE} dw.sys 1"));
                t.push(format!("tag {sel} remove {WIPED}"));
            }
            t.push(format!(
                "execute as {sel} run function {ns}:cp_respawn_check"
            ));
            for (i, axis) in ["x", "y", "z"].iter().enumerate() {
                t.push(format!(
                    "execute store result score #{axis}_cpseat dw.sys run data get entity {sel} \
                     Pos[{i}] 100"
                ));
            }
            t.push(format!(
                "assert score #x_cpseat dw.sys matches {}",
                cp.pos[0] * 100 + 50
            ));
            t.push(format!(
                "assert score #y_cpseat dw.sys matches {}",
                cp.pos[1] * 100
            ));
            t.push(format!(
                "assert score #z_cpseat dw.sys matches {}",
                cp.pos[2] * 100 + 50
            ));
            t.push(format!("assert score {sel} dw.death_ack matches 1"));
            // …and no second re-seat without a second death.
            t.push(format!(
                "tp {sel} {} {} {}",
                center(entry[0]),
                entry[1],
                center(entry[2])
            ));
            t.push(format!(
                "execute as {sel} run function {ns}:cp_respawn_check"
            ));
            t.push(format!(
                "execute store result score #x2_cpseat dw.sys run data get entity {sel} Pos[0] 100"
            ));
            t.push(format!(
                "assert score #x2_cpseat dw.sys matches {}",
                entry[0] * 100 + 50
            ));
            out.insert(
                format!("packtest-datapack/data/{ns}/test/v06_checkpoint_reseat.mcfunction"),
                lines(&t).into_bytes(),
            );

            // --- spec-0077: the respawn wait, on the same edge ---
            //
            // A party of two with the other member in play: the death edge puts
            // the player in spectator under the observation tag with the clock
            // at 1, and does NOT seat them; the release then seats them on the
            // checkpoint cell in adventure, untagged and unclocked.
            if respawn_wait(plan).is_some() {
                let (pin, sel) = pin_dummy("dw_t_cpwait");
                let mut t = packtest_header(&format!(
                    "{title}: a death in a party with somebody in play waits, and the \
                     release seats the player on the checkpoint (spec-0077)"
                ));
                t.push(format!("function {ns}:setup"));
                t.push(pin);
                t.push(format!("scoreboard players set #cp dw.sys {}", cp.index));
                t.push(format!("scoreboard players set {sel} dw.death_ack 0"));
                t.push(format!("scoreboard players set {sel} dw.deaths 1"));
                t.push(format!("tag {sel} remove {WIPED}"));
                t.push(format!(
                    "tp {sel} {} {} {}",
                    center(entry[0]),
                    entry[1],
                    center(entry[2])
                ));
                t.push(format!("scoreboard players set {RW_PRESENT} dw.sys 2"));
                t.push(format!("scoreboard players set {ALIVE} dw.sys 2"));
                t.push(format!(
                    "execute as {sel} run function {ns}:cp_respawn_check"
                ));
                t.push(format!(
                    "execute store success score #w_cpwait dw.sys if entity \
                     @a[tag=dw_t_cpwait,limit=1,gamemode=spectator,tag={CUTSCENE_TAG},scores={{{RW_CLOCK}=1}}]"
                ));
                t.push("assert score #w_cpwait dw.sys matches 1".to_string());
                t.push(format!(
                    "execute store result score #x_cpwait dw.sys run data get entity {sel} Pos[0] 100"
                ));
                t.push(format!(
                    "assert score #x_cpwait dw.sys matches {}",
                    entry[0] * 100 + 50
                ));
                t.push(format!("execute as {sel} run function {ns}:rw_release"));
                t.push(format!(
                    "execute store success score #r_cpwait dw.sys if entity \
                     @a[tag=dw_t_cpwait,limit=1,gamemode=adventure,tag=!{CUTSCENE_TAG}]"
                ));
                t.push("assert score #r_cpwait dw.sys matches 1".to_string());
                t.push(format!(
                    "execute store success score #c_cpwait dw.sys if score {sel} {RW_CLOCK} matches 1.."
                ));
                t.push("assert score #c_cpwait dw.sys matches 0".to_string());
                for (i, axis) in ["x", "z"].iter().enumerate() {
                    t.push(format!(
                        "execute store result score #{axis}s_cpwait dw.sys run data get entity {sel} \
                         Pos[{}] 100",
                        i * 2
                    ));
                }
                t.push(format!(
                    "assert score #xs_cpwait dw.sys matches {}",
                    cp.pos[0] * 100 + 50
                ));
                t.push(format!(
                    "assert score #zs_cpwait dw.sys matches {}",
                    cp.pos[2] * 100 + 50
                ));
                out.insert(
                    format!("packtest-datapack/data/{ns}/test/v06_checkpoint_wait.mcfunction"),
                    lines(&t).into_bytes(),
                );
            }
        }
    }

    if let Some(beat) = plan.stealth_beats.first() {
        let i = beat.index;
        let grace = beat.grace_ticks;
        let (_, zpos, zext) = &beat.zones[0];
        let inside = *zpos;
        let outside = [zpos[0] + zext[0] as i32 + 10, zpos[1], zpos[2]];
        let (pin, sel) = pin_dummy("dw_sttest");
        let mut t = packtest_header(&format!(
            "{title}: stealth catches the exposed, spares the hidden (spec-0014)"
        ));
        t.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`): the template teleports
        // it to absolute campaign coordinates, after which `@p` would resolve
        // to a neighbor test's dummy and the controlled state below would land
        // on — and be asserted against — the wrong player.
        t.push(pin);
        // --- spare: an in-zone player (zone presence alone = hidden) never
        //     accrues grace; an accrued grace is reset the moment they are safe. ---
        t.push(format!("function {ns}:stealth_begin_{i}"));
        // Disarm the live session marker `stealth_begin` just set: this test drives
        // `stealth_eval` explicitly, so the world `tick` loop (which runs
        // `stealth_eval` on every player while `#stealth` is armed) must NOT also
        // fire — a second judge pass in the same tick would double-count the
        // exposure (an extra grace increment per tick), corrupting the controlled
        // counts the asserts read. Runtime gameplay is unaffected (there the tick
        // loop is the sole caller); this only isolates the test.
        t.push("scoreboard players set #stealth dw.sys 0".to_string());
        t.push(format!("scoreboard players set {sel} dw.st_grace 5"));
        t.push(format!(
            "tp {sel} {} {} {}",
            inside[0], inside[1], inside[2]
        ));
        t.push(format!(
            "execute as {sel} run function {ns}:stealth_eval_{i}"
        ));
        t.push(format!("assert score {sel} dw.st_grace matches 0"));
        // --- caught: an exposed (out of every zone) player accrues grace and is
        //     caught on the grace_ticks-th judge tick (on_caught resets grace to
        //     0). This section runs LAST: the trip executes the campaign's real
        //     `on_caught`, whose effects are arbitrary content (the island's
        //     deals lethal damage) — nothing state-dependent may follow it, and
        //     the closing assert reads the dummy through the tag, which keeps
        //     matching even if `on_caught` killed it. ---
        t.push(format!("function {ns}:stealth_begin_{i}"));
        // Disarm again (this second `begin` re-armed `#stealth`); see note above.
        t.push("scoreboard players set #stealth dw.sys 0".to_string());
        t.push(format!(
            "tp {sel} {} {} {}",
            outside[0], outside[1], outside[2]
        ));
        // grace_ticks-1 judge ticks: grace climbs but has not yet tripped.
        for _ in 0..grace.saturating_sub(1) {
            t.push(format!(
                "execute as {sel} run function {ns}:stealth_eval_{i}"
            ));
        }
        t.push(format!(
            "assert score {sel} dw.st_grace matches {}",
            grace.saturating_sub(1)
        ));
        // One more tick trips on_caught, which resets grace to 0.
        t.push(format!(
            "execute as {sel} run function {ns}:stealth_eval_{i}"
        ));
        t.push(format!("assert score {sel} dw.st_grace matches 0"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_stealth.mcfunction"),
            lines(&t).into_bytes(),
        );

        // --- cutscene freeze (the staging invariant, see CUTSCENE_TAG): a player
        //     in the cutscene state is exposed — outside every zone — and must
        //     still NOT accrue grace while the marker is on, then must resume
        //     accruing the moment it comes off. Driven through the real
        //     `stealth_tick` gate (not `stealth_eval`), because the gate is what
        //     the freeze lives in.
        let (fpin, fsel) = pin_dummy("dw_t_cfrz");
        let mut f = packtest_header(&format!(
            "{title}: a cutscene freezes the stealth clock, and it resumes after"
        ));
        f.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`): the template tp's it to
        // absolute campaign coordinates, after which a bare `@p` would resolve
        // to a neighbor test's dummy — and an `@a` write (state, tp, or the
        // cutscene tag itself) would land on every dummy in the batch.
        f.push(fpin);
        f.push(format!("function {ns}:stealth_begin_{i}"));
        // Disarm the live session marker so the world `tick` loop does not judge
        // in the same tick; this test drives `stealth_tick` explicitly.
        f.push("scoreboard players set #stealth dw.sys 0".to_string());
        f.push(format!("scoreboard players set {fsel} dw.st_grace 0"));
        f.push(format!(
            "tp {fsel} {} {} {}",
            outside[0], outside[1], outside[2]
        ));
        f.push(format!("tag {fsel} add {CUTSCENE_TAG}"));
        // Well past `grace_ticks` of exposure: frozen, so grace stays 0.
        for _ in 0..grace + 2 {
            f.push(format!("function {ns}:stealth_tick_{i}"));
        }
        f.push(format!("assert score {fsel} dw.st_grace matches 0"));
        // Restore drops the marker; the clock resumes from where it paused.
        f.push(format!("tag {fsel} remove {CUTSCENE_TAG}"));
        for _ in 0..grace.saturating_sub(1) {
            f.push(format!("function {ns}:stealth_tick_{i}"));
        }
        f.push(format!(
            "assert score {fsel} dw.st_grace matches {}",
            grace.saturating_sub(1)
        ));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_cutscene_freeze.mcfunction"),
            lines(&f).into_bytes(),
        );
    }

    // damage-players: the `/damage` primitive the effect emits actually subtracts
    // health. A 0-player void does not tick a real player, so the test drives the
    // damage on a summoned dummy (NoAI/Silent zombie, full 20 HP) with the exact
    // amount + type the first declared `damage-players` uses, then asserts its
    // Health dropped by that amount. Emitted only when the campaign uses the verb.
    if let Some((amount, kind)) = first_damage_players(plan.campaign) {
        let type_id = kind.id();
        let mut t = packtest_header(&format!(
            "{title}: damage-players subtracts {amount} half-hearts ({type_id}) (spec-0014)"
        ));
        t.push(format!("function {ns}:setup"));
        // A body where this template's own PackTest dummy stands: NoAI so it
        // never moves, Silent, full health. `damage` applies synchronously, so a
        // 0-player void still shows it. Summoned at the dummy — the nearest
        // player on the template's first line, inside its own loaded test
        // structure — and never at a fixed cell: a fixed cell near origin is
        // loaded or not by how the batch happens to lay its structures out, and
        // a batch of 19 left `0 -60 0` where the summon put nothing a `damage`
        // could find (both reads 0, the drop 0). Pre-clear the tag first —
        // never assume a fresh world on the shared-batch server — and kill again
        // on the way out.
        t.push("kill @e[tag=dw_dmgtest]".to_string());
        t.push(
            "execute at @p run summon minecraft:zombie ~ ~ ~ {Tags:[\"dw_dmgtest\"],NoAI:1b,\
             Silent:1b,PersistenceRequired:1b,Health:20f}"
                .to_string(),
        );
        t.push(
            "execute store result score #hp0_dmg dw.sys run data get entity \
             @e[tag=dw_dmgtest,limit=1] Health 100"
                .to_string(),
        );
        t.push(format!(
            "damage @e[tag=dw_dmgtest,limit=1] {amount} {type_id}"
        ));
        t.push(
            "execute store result score #hp1_dmg dw.sys run data get entity \
             @e[tag=dw_dmgtest,limit=1] Health 100"
                .to_string(),
        );
        // The dummy's Health (×100) must have dropped: drop = hp0 - hp1 ≥ 1. Asserting
        // "strictly decreased" rather than an exact amount keeps the test robust across
        // damage types (armor-respecting types reduce the number, but the hit still
        // lands); the exact `damage @s <amount> <type>` string is asserted by a
        // compiler unit test.
        t.push("scoreboard players operation #drop_dmg dw.sys = #hp0_dmg dw.sys".to_string());
        t.push("scoreboard players operation #drop_dmg dw.sys -= #hp1_dmg dw.sys".to_string());
        t.push("assert score #drop_dmg dw.sys matches 1..".to_string());
        t.push("kill @e[tag=dw_dmgtest]".to_string());
        out.insert(
            format!("packtest-datapack/data/{ns}/test/v06_damage.mcfunction"),
            lines(&t).into_bytes(),
        );
    }
}

/// The `(amount, damage_type)` of the first `damage-players` effect declared in the
/// campaign (deep-walked through nested effect lists), in quest-then-trigger order.
/// `None` when the campaign uses no `damage-players`. Drives the damage PackTest.
fn first_damage_players(
    c: &delvewright_dsl::Campaign,
) -> Option<(u32, delvewright_dsl::DamageKind)> {
    use delvewright_dsl::DamageKind;
    let mut found: Option<(u32, DamageKind)> = None;
    let mut scan = |eff: &QuestEffect| {
        if found.is_none() {
            eff.visit_deep(&mut |e| {
                if found.is_none()
                    && let Verb::DamagePlayers {
                        amount,
                        damage_type,
                        ..
                    } = &e.verb
                {
                    found = Some((*amount, damage_type.unwrap_or(DamageKind::Generic)));
                }
            });
        }
    };
    // Every root, inherited. Strictly additive: the roots keep their order, so the
    // "first" effect is unchanged for any campaign that has one in R1-R3 — this only
    // ever finds a `damage-players` where the generator previously found none and
    // emitted no damage PackTest at all.
    crate::compiler::plan::for_each_effect_root(c, &mut |_site, effs| {
        for eff in effs {
            scan(eff);
        }
    });
    found
}
