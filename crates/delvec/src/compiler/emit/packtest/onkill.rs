use super::*;

/// A kill pays (spec-0074 §6, acceptance criterion 3): one set of templates per
/// fight that declares an `on_kill`, driven with the real generated functions
/// and the template's own PackTest dummy.
///
/// **The credited blow is `damage … minecraft:player_attack by <dummy>`**, and
/// that is a measurement, not an assumption: on the pinned 1.21.11 server, a
/// fake player named as the `by` source of a lethal `damage` is granted
/// `minecraft:player_killed_entity`, the kill advancement's reward runs as it on
/// tick 0, and the same `damage` without `by` credits nobody. So every template
/// reaches the bundle the way a player does — through vanilla's own credit and
/// the real advancement — rather than by granting the advancement by hand.
/// Every blow selects living bodies only (`nbt=!{Health:0.0f}`): a killed body
/// stays in the world for its death animation, and a second blow at a corpse
/// would be a blow at nothing.
///
/// Per fight `<f>` (`w_<wave>` or `a_<actor>`), ledger `#kf_<f>`:
///
/// * `kill_pays_<f>` — one credited blow pays once: the ledger is 1 and the
///   bundle's first ungated `add-state` holder moved by its amount (the party's
///   `#party`, or the dummy's own score for a `player` datum); then the bundle
///   body driven directly (`DW0811`'s `kill-pays` claim) pays a second time;
/// * `kill_pays_uncredited_<f>` — every body dies with nobody credited and the
///   real `tick` runs: the ledger is still 0. (That the fight still clears is
///   `verb_kill_uncredited`'s claim; asserting the countdown here would make every
///   gate that can reach a spawn this template's business, `DW0807`.)
/// * `kill_pays_removed_<f>` — every removal the compiler owns for this fight,
///   run over standing bodies (a rest's re-seat through the real
///   `bonfire_rest_<i>`, which runs `wave_reseat_<wave>` / `actor_restand_<actor>`;
///   an actor's `unleash_<actor>` puppet swap; the `despawn-actor` lines of both
///   styles): the ledger is still 0. Emitted where the fight has a removal to
///   run;
/// * `kill_pays_across_rest_<f>` — only where a rest brings the fight back:
///   kill, run the real `bonfire_rest_<i>`, kill again, and the ledger tells the
///   two `fires` values apart. A `respawns_on_rest` wave comes back whether it
///   was beaten or not, so every body dies twice: `first-kill` pays N and
///   `every-kill` 2N. A fight re-seated only while it still stands (an
///   `elite`/`boss` wave, an unleashed actor) is left one body standing before
///   the rest, or the rest brings back nothing: `first-kill` pays N and
///   `every-kill` 2N − 1 — which for a one-body fight is 1 either way, the
///   honest answer for a body that is never killed twice.
pub(super) fn emit_kill_pays_packtests(
    plan: &Plan,
    out: &mut BuildOutput,
    wave_placements: &WavePlacements,
) {
    use crate::compiler::onkill::ComesBack;
    use delvewright_dsl::Fight;
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let mut fights: Vec<(Fight<'_>, &delvewright_dsl::OnKill)> =
        wave_machinery_waves(plan, wave_placements)
            .filter_map(|w| w.on_kill.as_ref().map(|ok| (Fight::Wave(w), ok)))
            .collect();
    fights.extend(
        on_kill_actors(plan)
            .into_iter()
            .map(|(a, ok)| (Fight::Actor(a), ok)),
    );
    if fights.is_empty() {
        return;
    }
    // Every fight a rest can re-seat, cleared on entry and exit by any template
    // that runs a rest, so the rest re-seats this fight and nothing else
    // (pin_dummy rule 4).
    let mut board: Vec<String> = Vec::new();
    for w in plan.reseat_waves() {
        board.push(format!("kill @e[tag={}]", plan::wave_tag(w.id.as_str())));
        board.push(format!(
            "scoreboard players set {} dw.sys 0",
            wave_seated_holder(w.id.as_str())
        ));
    }
    for w in plan.undefeated_reseat_waves() {
        board.push(format!("kill @e[tag={}]", plan::wave_tag(w.id.as_str())));
    }
    for a in plan.reseat_actors() {
        board.push(format!(
            "kill @e[tag=dw_actor_{}]",
            plan::safe_local(a.id.as_str())
        ));
    }
    let hostile: BTreeSet<&str> = delvewright_dsl::unleashed_actors(plan.campaign);
    for (fight, ok) in fights {
        let f = kill_ledger(fight).trim_start_matches("#kf_").to_string();
        let ledger = kill_ledger(fight);
        let bodies = fight_bodies(fight);
        let (tag, seat): (String, Vec<String>) = match fight {
            Fight::Wave(w) => {
                let safe = plan::safe_local(w.id.as_str());
                (
                    plan::wave_tag(w.id.as_str()),
                    vec![format!("function {ns}:spawn_{safe}")],
                )
            }
            Fight::Actor(a) => {
                let safe = plan::safe_local(a.id.as_str());
                let mut seat = vec![format!("function {ns}:spawn_actor_{safe}")];
                if hostile.contains(a.id.as_str()) {
                    seat.push(format!("function {ns}:unleash_{safe}"));
                }
                (format!("dw_actor_{safe}"), seat)
            }
        };
        let living = format!("@e[tag={tag},nbt=!{{Health:0.0f}}]");
        let tpin = format!("dw_t_kp_{f}");
        let (pin, sel) = pin_dummy(&tpin);
        let credited = |limit: Option<i32>| {
            let sel_bodies = match limit {
                Some(n) => format!("@e[tag={tag},nbt=!{{Health:0.0f}},limit={n}]"),
                None => living.clone(),
            };
            format!("execute as {sel_bodies} run damage @s 1000 minecraft:player_attack by {sel}")
        };
        // A wave's countdown is batch-global: handed back at the value a fresh
        // spawn leaves (pin_dummy rule 4).
        let restore: Vec<String> = match fight {
            Fight::Wave(w) => vec![format!(
                "scoreboard players set {} {} {}",
                plan::wave_counter(w.id.as_str()),
                plan::WAVE_OBJECTIVE,
                plan::wave_total(w)
            )],
            Fight::Actor(_) => Vec::new(),
        };
        let write = |name: &str, b: Vec<String>, out: &mut BuildOutput| {
            out.insert(
                format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
                lines(&b).into_bytes(),
            );
        };

        // --- kill_pays_<f>: one credited blow pays once ---
        // The bundle's first ungated `add-state`, asserted beside the ledger.
        let paid = ok.effects.iter().find_map(|e| match &e.verb {
            Verb::AddState { state, amount, .. } if e.when.is_none() => {
                let holder = match state_holder(plan, state).as_str() {
                    "@s" => sel.clone(),
                    h => h.to_string(),
                };
                Some((holder, plan::state_score(state.as_str()), *amount))
            }
            _ => None,
        });
        let mut b = packtest_header(&format!(
            "{title}: a credited kill of {} `{}` pays its `on_kill` once (spec-0074)",
            fight.word(),
            fight.id()
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("scoreboard players set {ledger} dw.sys 0"));
        if let Some((holder, obj, _)) = &paid {
            b.push(format!("scoreboard players set {holder} {obj} 0"));
        }
        b.extend(seat.iter().cloned());
        b.push(credited(Some(1)));
        b.push(format!("assert score {ledger} dw.sys matches 1"));
        if let Some((holder, obj, amount)) = &paid {
            b.push(format!("assert score {holder} {obj} matches {amount}"));
        }
        // The body driven directly — the per-object drive `DW0811` judges, and
        // the half a credited blow cannot stand in for: it proves THIS fight's
        // own bundle function increments THIS fight's own ledger.
        b.push(format!(
            "execute as {sel} run function {ns}:{}",
            on_kill_function(fight)
        ));
        b.push(format!("assert score {ledger} dw.sys matches 2"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("scoreboard players set {ledger} dw.sys 0"));
        if let Some((holder, obj, _)) = &paid {
            b.push(format!("scoreboard players set {holder} {obj} 0"));
        }
        b.extend(restore.iter().cloned());
        b.push(format!("tag {sel} remove {tpin}"));
        write(&format!("kill_pays_{f}"), b, out);

        // --- kill_pays_uncredited_<f>: a death nobody is credited with ---
        let mut b = packtest_header(&format!(
            "{title}: {} `{}` dying with no player credited pays nothing, and the fight \
             still clears (spec-0074 §3)",
            fight.word(),
            fight.id()
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("scoreboard players set {ledger} dw.sys 0"));
        b.extend(seat.iter().cloned());
        b.push(format!(
            "execute as {living} run damage @s 1000 minecraft:generic"
        ));
        b.push(format!("function {ns}:tick"));
        // The ledger only. The countdown is deliberately NOT asserted after the
        // real `tick`: `spawn_<wave>` writes it too, so asserting it would make
        // every `#party` gate that can reach a spawn this template's business
        // (`DW0807`) — the reason `verb_kill_uncredited` asserts its objective
        // instead, which is where "the fight still clears" is proven.
        b.push(format!("assert score {ledger} dw.sys matches 0"));
        b.push(format!("kill @e[tag={tag}]"));
        b.extend(restore.iter().cloned());
        b.push(format!("tag {sel} remove {tpin}"));
        write(&format!("kill_pays_uncredited_{f}"), b, out);

        // --- kill_pays_removed_<f>: no compiler removal pays ---
        // A rest's re-seat is reached through the real `bonfire_rest_<i>` — the
        // one path that runs it in play — rather than by calling
        // `wave_reseat_<wave>` / `actor_restand_<actor>` directly: a direct call
        // would drive this fight's re-seat body and leave its un-bundled
        // siblings undriven, which is `DW0810`'s finding against the suite.
        let rest = plan.bonfires().next().map(|bf| bf.index);
        let reseated = match fight {
            Fight::Wave(w) => {
                w.respawns_on_rest || plan.undefeated_reseat_waves().iter().any(|u| u.id == w.id)
            }
            Fight::Actor(a) => plan.reseat_actors().iter().any(|x| x.id == a.id),
        };
        let mut removals: Vec<Vec<String>> = Vec::new();
        if let Some(i) = rest.filter(|_| reseated) {
            removals.push(vec![format!("function {ns}:bonfire_rest_{i}")]);
        }
        if let Fight::Actor(a) = fight {
            for style in [
                delvewright_dsl::DespawnStyle::Kill,
                delvewright_dsl::DespawnStyle::Vanish,
            ] {
                let mut lines_ = Vec::new();
                emit_despawn_actor(
                    ns,
                    a.id.as_str(),
                    style,
                    actor_declares_drops(a),
                    &mut lines_,
                );
                removals.push(lines_);
            }
        }
        if !removals.is_empty() {
            let mut b = packtest_header(&format!(
                "{title}: no removal the compiler performs on {} `{}` pays its `on_kill` \
                 (spec-0074 §3)",
                fight.word(),
                fight.id()
            ));
            b.push(format!("function {ns}:setup"));
            b.push(pin.clone());
            b.extend(board.iter().cloned());
            b.push(format!("kill @e[tag={tag}]"));
            b.push(format!("scoreboard players set {ledger} dw.sys 0"));
            for removal in &removals {
                // Every removal runs over bodies that stand: seated afresh first.
                b.push(format!("kill @e[tag={tag}]"));
                b.extend(seat.iter().cloned());
                b.push(format!(
                    "execute store result score #kpr_{f} dw.sys if entity {living}"
                ));
                b.push(format!("assert score #kpr_{f} dw.sys matches 1.."));
                // Brand the standing bodies, so the template can see the removal
                // really removed THEM — a removal that did nothing would also
                // pay nothing.
                b.push(format!("tag {living} add dw_kpr_{f}"));
                // Onto the dummy, so an unseen removal parks them in its column.
                b.push(format!("execute at {sel} run tp {living} ~ ~ ~"));
                b.extend(removal.iter().cloned());
                // An unseen removal ([`Exit::Unseen`]) only moves the body under
                // the world and takes its tags, the brand with them; it dies when
                // the sweep runs, a few ticks on. The bodies are counted waiting
                // in the dummy's column, then killed there in the same tick
                // ([`unseen_sweep_under`]), so the ledger is read after the
                // removal's death and not before it — a removal whose death were
                // credited would red this template.
                let sweep = format!("schedule function {ns}:{UNSEEN_SWEEP_FN} ");
                if removal
                    .iter()
                    .any(|l| l.contains(&sweep) || l.contains(":bonfire_rest_"))
                {
                    let waiting = format!("@e[tag={UNSEEN_TAG},distance=..1,nbt=!{{Health:0.0f}}]");
                    let low = format!("execute at {sel} positioned ~ {UNSEEN_Y} ~");
                    b.push(format!(
                        "{low} store result score #kpu_{f} dw.sys if entity {waiting}"
                    ));
                    b.push(format!("assert score #kpu_{f} dw.sys matches 1.."));
                    b.push(unseen_sweep_under(&sel));
                    b.push(format!(
                        "{low} store result score #kpu_{f} dw.sys if entity {waiting}"
                    ));
                    b.push(format!("assert score #kpu_{f} dw.sys matches 0"));
                }
                b.push(format!(
                    "execute store result score #kpr_{f} dw.sys if entity \
                     @e[tag=dw_kpr_{f},nbt=!{{Health:0.0f}}]"
                ));
                b.push(format!("assert score #kpr_{f} dw.sys matches 0"));
                b.push(format!("assert score {ledger} dw.sys matches 0"));
            }
            b.push(format!("kill @e[tag={tag}]"));
            b.extend(board.iter().cloned());
            b.extend(restore.iter().cloned());
            b.push(format!("tag {sel} remove {tpin}"));
            write(&format!("kill_pays_removed_{f}"), b, out);
        }

        // --- kill_pays_across_rest_<f>: the two `fires` values, told apart ---
        let back = plan.fight_comes_back(fight);
        let Some(bf) = plan.bonfires().next() else {
            continue;
        };
        let every = ok.fires == Some(delvewright_dsl::KillFires::EveryKill);
        let (first_round, expect): (Option<i32>, i32) = match back {
            Some(ComesBack::RespawnsOnRest) => (None, if every { 2 * bodies } else { bodies }),
            Some(ComesBack::Undefeated(_)) | Some(ComesBack::Unleashed) => (
                Some(bodies - 1),
                if every { 2 * bodies - 1 } else { bodies },
            ),
            Some(ComesBack::Repeats { .. }) | None => continue,
        };
        let mut b = packtest_header(&format!(
            "{title}: {} `{}` comes back after a rest and pays `{}` (spec-0074 §4)",
            fight.word(),
            fight.id(),
            ok.fires
                .map(delvewright_dsl::KillFires::token)
                .unwrap_or("first-kill")
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        b.extend(board.iter().cloned());
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("scoreboard players set {ledger} dw.sys 0"));
        b.extend(seat.iter().cloned());
        match first_round {
            None => b.push(credited(None)),
            Some(0) => {}
            Some(n) => b.push(credited(Some(n))),
        }
        b.push(format!("function {ns}:bonfire_rest_{}", bf.index));
        b.push(format!(
            "execute store result score #kpa_{f} dw.sys if entity {living}"
        ));
        b.push(format!("assert score #kpa_{f} dw.sys matches {bodies}"));
        b.push(credited(None));
        b.push(format!("assert score {ledger} dw.sys matches {expect}"));
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("scoreboard players set {ledger} dw.sys 0"));
        b.extend(board.iter().cloned());
        b.extend(restore.iter().cloned());
        b.push(format!("tag {sel} remove {tpin}"));
        write(&format!("kill_pays_across_rest_{f}"), b, out);
    }
}

/// `DW0811`'s claim over the `on_kill` bundles (spec-0074): every fight that
/// declares one has its own bundle body driven by its own `kill_pays_<f>`.
pub(super) fn kill_pays_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    let content = &plan.campaign.quests.content;
    crate::compiler::watch::Claim {
        mechanic: "kill-pays",
        families: vec!["on_kill_w_".to_string(), "on_kill_a_".to_string()],
        declared: content
            .waves
            .iter()
            .filter(|w| w.on_kill.is_some())
            .map(|w| plan::safe_local(w.id.as_str()))
            .chain(
                content
                    .actors
                    .iter()
                    .filter(|a| a.on_kill.is_some())
                    .map(|a| plan::safe_local(a.id.as_str())),
            )
            .collect(),
    }
}
