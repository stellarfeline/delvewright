use super::*;

/// spec-0021 loot PackTest: after `setup`, the declared container really holds
/// the declared stacks.
///
/// The contract worth proving on a real server is that `item replace block …
/// container.<n>` LANDED — it is the command that fails silently on a
/// non-container, and `DW0431` proves the container exists at compile time but
/// cannot prove the fill took. Asserts the first and last slot of the first
/// declared fill, by id, so a positional-slot regression is caught too.
/// Emitted only for a campaign that declares `loot` (else byte-identical).
/// The cast-ledger PackTests (spec-0020 acceptance): one `cast_ladder_<npc>`
/// per ledger NPC proving WHICH scene the selector picks per clause, one
/// `cast_bark_cycle` covering every bark pool, one `cast_none_silent` covering
/// every declared `"none"` scene.
///
/// ## Per clause, never per exemplar
///
/// The shape this replaces searched the ledgers for the first NPC whose ledger
/// fit the template ("two root scenes", "a pool of two") and proved that one —
/// so a campaign whose ledgers fit differently had NO proof at all (the
/// engine's own gallery: four NPCs, eight clauses, zero root-swap proofs,
/// because no NPC held two ROOT scenes), and the search that skipped a clause
/// was the same search that chose the exemplar. The dialogue-mask loop
/// (`emit_one_dialogue_mask_packtest`) records the identical lesson. Here every
/// clause of every ledger is driven, and the walk is registered as two
/// `watch::Claim`s (`cast-ladder` over `cast_`, `cast-bark` over each NPC's
/// `bark_<npc>_`) so a walk that quietly stops short of the declared list is a
/// `DW0811` refusal, not a smaller green.
///
/// ## The drive is SOLVED, not assumed
///
/// Asserting "clause k's scene" is only a proof if the driven state actually
/// selects clause k — later clauses of the same quest override, so a setup that
/// pins k's own terms and nothing else can be overridden by a sibling clause
/// and assert the wrong scene (two clauses of one quest differing only by
/// `requires_state` are exactly the case a flag pin cannot distinguish). So the
/// drive comes from [`crate::compiler::cast::distinguishing_drive`]: k's gate satisfied,
/// every later same-quest clause's gate violated, every score the ladder reads
/// pinned (island r15: the batch is one shared server, and a term left undriven
/// is decided by whichever sibling ran last). A clause with no such drive never
/// governs at any runtime state and is refused upstream (`DW0846`), which is
/// why there is no exemption path here: the one condition under which this
/// loop cannot prove a clause is a build that already failed.
///
/// ## Both directions, from one model
///
/// Each clause is asserted selected under its drive, and then every term of its
/// own gate is broken one at a time (restored after), with the expected
/// selection re-computed by [`crate::compiler::cast::eval_ladder`] — the compiler-side
/// model of the emitted selector, read off the same authored ledger the emitter
/// walks. A selector that lost a clause, dropped a gate axis (the
/// `requires_state` condition, say) or scrambled its order disagrees with the
/// model at one of these points ON THE LIVE SERVER, which is what makes the
/// suite red when the emitted body is silently wrong. Stated limit: the assert
/// is on the SELECTION (`dw.cast`), so at a driven point where the model says
/// breaking a term leaves the selection unchanged (an `"unchanged"` chain can
/// give two adjacent clauses one scene), that term's loss is unobservable there
/// — because it is unobservable to the player there too.
///
/// All ladder phases drive `cast_<npc>` (pure scoreboard math) rather than
/// `talk_<npc>` — a dummy has no client to show a dialog to. The `"none"`
/// phases drive `talk_<npc>` itself, precisely because a silent scene emits no
/// `dialog show`: that is what makes "the record is written and consumed, and
/// nothing opens" directly assertable.
pub(super) fn emit_cast_packtests(plan: &Plan, out: &mut BuildOutput) {
    let casts = crate::compiler::cast::npc_casts(plan.campaign);
    for npc in &plan.npcs {
        if let Some(cast) = casts.get(&npc.npc_id) {
            emit_cast_ladder_packtest(plan, npc, cast, out);
        }
    }
    emit_cast_bark_packtest(plan, &casts, out);
    emit_cast_silent_packtest(plan, &casts, out);
}

/// The pin lines that put the shared batch server into `drive`'s state for one
/// NPC's ladder: every quest-active score the ladder dispatches on, every
/// branch flag and every datum it reads — in that order, so the emitted text
/// reads as "story, then branch, then meter". `sel` is the template's own
/// pinned dummy, which is the acting player a `player`-scoped datum belongs to
/// (`GateConsumer::CastPlacement` evaluates per player).
fn cast_drive_lines(
    plan: &Plan,
    cast: &crate::compiler::cast::NpcCast,
    drive: &crate::compiler::cast::ClauseDrive,
    sel: &str,
) -> Vec<String> {
    let mut b = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for cl in &cast.by_quest {
        if seen.insert(cl.quest.as_str()) {
            b.push(format!(
                "scoreboard players set {} {} {}",
                plan::PARTY,
                quest_active_score(&cl.quest),
                i32::from(drive.begun.contains(&cl.quest))
            ));
        }
    }
    for (f, v) in &drive.flags {
        b.push(format!(
            "scoreboard players set {} {} {v}",
            plan::PARTY,
            plan::flag_score(f)
        ));
    }
    for (s, v) in &drive.datums {
        b.push(format!(
            "scoreboard players set {} {} {v}",
            cast_state_holder(plan, sel, s),
            plan::state_score(s)
        ));
    }
    b
}

/// Who holds a datum in a generated cast template: the party fake player, or
/// the template's own pinned dummy for a `player`-scoped datum. The dummy is
/// named outright rather than through `@s` because these pin lines run at the
/// template's top level, where there is no acting player to be `@s`.
fn cast_state_holder(plan: &Plan, sel: &str, id: &str) -> String {
    match plan.campaign.quests.content.state_decl(id).map(|s| s.scope) {
        Some(StateScope::Player) => sel.to_string(),
        _ => plan::PARTY.to_string(),
    }
}

/// One NPC's ladder proof: `cast_ladder_<npc>.mcfunction`.
///
/// Phase 0 drives the empty story (every quest inactive, every flag 0, every
/// datum 0) and asserts scene 0 — the "no declaring quest has begun" row.
/// Then, per clause in ladder order: drive its distinguishing state, run the
/// real `cast_<npc>`, assert the clause's own scene; then break each term of
/// its gate one at a time and assert the model's answer for the broken state,
/// restoring after each. See `emit_cast_packtests` for why the drive is solved
/// and what the negative phases catch.
fn emit_cast_ladder_packtest(
    plan: &Plan,
    npc: &plan::NpcPlan,
    cast: &crate::compiler::cast::NpcCast,
    out: &mut BuildOutput,
) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let safe = &npc.safe;
    let (pin, sel) = pin_dummy(&format!("dw_t_clad_{safe}"));
    let mut b = packtest_header(&format!(
        "{title}: npc `{}`'s cast ladder selects each declared clause's own scene",
        npc.npc_id
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    b.push("# Every phase pins EVERYTHING the ladder reads — quest actives,".to_string());
    b.push("# branch flags, datums. The batch is one shared server; a term left".to_string());
    b.push("# undriven is decided by whichever sibling template ran last.".to_string());

    let run = format!("execute as {sel} run function {ns}:cast_{safe}");
    let assert = |b: &mut Vec<String>, scene: u32| {
        b.push(format!("assert score {sel} {CAST_SCORE} matches {scene}"));
    };

    // Phase 0: the empty story.
    let empty = crate::compiler::cast::ClauseDrive {
        begun: BTreeSet::new(),
        flags: crate::compiler::cast::ladder_flag_reads(cast)
            .into_iter()
            .map(|f| (f, 0))
            .collect(),
        datums: crate::compiler::cast::ladder_datum_reads(cast)
            .into_iter()
            .map(|s| (s, 0))
            .collect(),
    };
    b.push("# -- no declaring quest has begun: the stage-6 root governs (scene 0) --".to_string());
    b.extend(cast_drive_lines(plan, cast, &empty, &sel));
    b.push(run.clone());
    assert(&mut b, 0);

    for (n, cl) in cast.by_quest.iter().enumerate() {
        let Some(drive) = crate::compiler::cast::distinguishing_drive(cast, n) else {
            // Unreachable in a validated build: a clause with no distinguishing
            // state is refused as DW0846 (shadowed) or DW0847 (its own gate
            // never opens) before emission. Named rather than silent so a tree
            // that bypassed validation still says what is missing.
            b.push(format!(
                "# -- clause {}: quest `{}` scene {} has NO distinguishing state (DW0846/DW0847) --",
                n + 1,
                cl.quest,
                cl.scene
            ));
            continue;
        };
        b.push(format!(
            "# -- clause {}: quest `{}` placement {} -> scene {} --",
            n + 1,
            cl.quest,
            cl.placement,
            cl.scene
        ));
        b.extend(cast_drive_lines(plan, cast, &drive, &sel));
        b.push(run.clone());
        assert(&mut b, cl.scene);

        // Break each term of THIS clause's gate on its own, assert the model's
        // answer, restore. One term at a time: broken together, a selector that
        // reads only the first term passes just as well.
        for f in &cl.requires_flags {
            let mut flags = drive.flags.clone();
            flags.insert(f.clone(), 0);
            let expect =
                crate::compiler::cast::eval_ladder(cast, &drive.begun, &flags, &drive.datums);
            b.push(format!(
                "# required `{f}` broken: the ladder must fall to scene {expect}"
            ));
            b.push(format!(
                "scoreboard players set {} {} 0",
                plan::PARTY,
                plan::flag_score(f)
            ));
            b.push(run.clone());
            assert(&mut b, expect);
            b.push(format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                plan::flag_score(f)
            ));
        }
        for f in &cl.forbids_flags {
            let mut flags = drive.flags.clone();
            flags.insert(f.clone(), 1);
            let expect =
                crate::compiler::cast::eval_ladder(cast, &drive.begun, &flags, &drive.datums);
            b.push(format!(
                "# forbidden `{f}` raised: the ladder must fall to scene {expect}"
            ));
            b.push(format!(
                "scoreboard players set {} {} 1",
                plan::PARTY,
                plan::flag_score(f)
            ));
            b.push(run.clone());
            assert(&mut b, expect);
            b.push(format!(
                "scoreboard players set {} {} 0",
                plan::PARTY,
                plan::flag_score(f)
            ));
        }
        for t in &cl.requires_state {
            let broken = state_drive_value(t, false);
            let mut datums = drive.datums.clone();
            datums.insert(t.state.as_str().to_string(), broken);
            let expect =
                crate::compiler::cast::eval_ladder(cast, &drive.begun, &drive.flags, &datums);
            let holder = cast_state_holder(plan, &sel, t.state.as_str());
            let obj = plan::state_score(t.state.as_str());
            b.push(format!(
                "# `{}` {} {} broken (driven to {broken}): the ladder must fall to scene {expect}",
                t.state.as_str(),
                t.op.token(),
                t.value
            ));
            b.push(format!("scoreboard players set {holder} {obj} {broken}"));
            b.push(run.clone());
            assert(&mut b, expect);
            b.push(format!(
                "scoreboard players set {holder} {obj} {}",
                drive.datums[t.state.as_str()]
            ));
        }
    }

    out.insert(
        format!("packtest-datapack/data/{ns}/test/cast_ladder_{safe}.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// Every bark pool of every ledger NPC cycles deterministically:
/// `cast_bark_cycle.mcfunction`, one sequence of phases per pool.
///
/// A bark body is per (NPC, scene) — its own pool, its own counter, its own
/// lines — so a proof over one pool says nothing about the next
/// (`crate::compiler::watch`'s whole lesson). Single-line pools are covered too: their
/// cycle property is "always line 1", asserted by running twice.
fn emit_cast_bark_packtest(
    plan: &Plan,
    casts: &std::collections::BTreeMap<String, crate::compiler::cast::NpcCast>,
    out: &mut BuildOutput,
) {
    use crate::compiler::cast::SceneAction;
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let (pin, sel) = pin_dummy("dw_t_castbark");
    let mut b = packtest_header(&format!(
        "{title}: every bark pool cycles deterministically through its own lines"
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    let mut pools = 0usize;
    for npc in &plan.npcs {
        let Some(cast) = casts.get(&npc.npc_id) else {
            continue;
        };
        for scene in &cast.scenes {
            let SceneAction::Barks(pool) = &scene.action else {
                continue;
            };
            pools += 1;
            let n = pool.len();
            let holder = format!("#bk_{}_{}", npc.safe, scene.index);
            b.push(format!(
                "# -- npc `{}` scene {}: {n} line(s) --",
                npc.npc_id, scene.index
            ));
            b.push("# The pool counter is shared runtime state: initialize it.".to_string());
            b.push(format!("scoreboard players set {holder} dw.sys 0"));
            for i in 1..=n {
                b.push(format!(
                    "execute as {sel} run function {ns}:bark_{}_{}",
                    npc.safe, scene.index
                ));
                b.push(format!("assert score {holder} dw.sys matches {i}"));
            }
            b.push("# One more right-click wraps to the first line — never RNG.".to_string());
            b.push(format!(
                "execute as {sel} run function {ns}:bark_{}_{}",
                npc.safe, scene.index
            ));
            b.push(format!("assert score {holder} dw.sys matches 1"));
        }
    }
    if pools == 0 {
        return;
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/cast_bark_cycle.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// Every declared `"none"` scene answers with nothing:
/// `cast_none_silent.mcfunction`, one sequence of phases per silent scene.
///
/// Drives `talk_<npc>` itself (the one place a silent scene's "nothing opens"
/// is observable), through the same solved drive the ladder proof uses — the
/// scene must actually be SELECTED, and a flag pin alone cannot guarantee that
/// against a sibling clause differing only in `requires_state`.
fn emit_cast_silent_packtest(
    plan: &Plan,
    casts: &std::collections::BTreeMap<String, crate::compiler::cast::NpcCast>,
    out: &mut BuildOutput,
) {
    use crate::compiler::cast::SceneAction;
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    let (pin, sel) = pin_dummy("dw_t_castnone");
    let mut b = packtest_header(&format!(
        "{title}: every `\"none\"` scene consumes the interaction and opens nothing"
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    let mut scenes = 0usize;
    for npc in &plan.npcs {
        let Some(cast) = casts.get(&npc.npc_id) else {
            continue;
        };
        for scene in &cast.scenes {
            if scene.action != SceneAction::Silent {
                continue;
            }
            // The first clause that can be distinguished while selecting this
            // scene. Every clause has one in a validated build (DW0846/DW0847).
            let Some((n, _)) = cast.by_quest.iter().enumerate().find(|(n, cl)| {
                cl.scene == scene.index
                    && crate::compiler::cast::distinguishing_drive(cast, *n).is_some()
            }) else {
                continue;
            };
            let drive = crate::compiler::cast::distinguishing_drive(cast, n)
                .expect("chosen because a drive exists");
            scenes += 1;
            let safe = &npc.safe;
            let idx = scene.index;
            b.push(format!(
                "# -- npc `{}` scene {idx} (`\"none\"`, declared by `{}`) --",
                npc.npc_id, scene.declared_by
            ));
            b.extend(cast_drive_lines(plan, cast, &drive, &sel));
            b.push("# Grant the interaction advancement, exactly as a right-click".to_string());
            b.push("# does: the record is written.".to_string());
            b.push(format!(
                "execute as {sel} run advancement grant @s only {ns}:{safe}_interact"
            ));
            b.push("# Run the reward the advancement fires. It must revoke (consume)".to_string());
            b.push("# the record and, for a silent scene, do nothing else.".to_string());
            b.push(format!("execute as {sel} run function {ns}:talk_{safe}"));
            b.push(format!("assert score {sel} {CAST_SCORE} matches {idx}"));
            b.push("# The record is consumed, so the advancement is re-armed: a".to_string());
            b.push("# second right-click still works (no dead NPC).".to_string());
            // Vanilla has no `execute if advancement`; the selector argument is
            // the primitive for reading advancement state.
            b.push(format!(
                "scoreboard players set #cnone_{safe}_{idx} dw.sys 0"
            ));
            b.push(format!(
                "execute as {sel} if entity @s[advancements={{{ns}:{safe}_interact=false}}] run \
                 scoreboard players set #cnone_{safe}_{idx} dw.sys 1"
            ));
            b.push(format!("assert score #cnone_{safe}_{idx} dw.sys matches 1"));
            // Deliberately NOT asserted here: that the bark counter did not
            // move. It is a shared runtime holder the bark-cycle template
            // drives over the same ticks — asserting it from two templates is
            // exactly the interleaving dependence `packtest_batch` forbids. "A
            // silent scene emits no action clause" is a property of the emitted
            // text, proved in `tests/cast_emit.rs` where it is race-free by
            // construction.
        }
    }
    if scenes == 0 {
        return;
    }
    out.insert(
        format!("packtest-datapack/data/{ns}/test/cast_none_silent.mcfunction"),
        lines(&b).into_bytes(),
    );
}

/// The claim the cast-ladder loop makes, judged by `DW0811`: every NPC the
/// campaign gave a cast ledger has its `cast_<npc>` selector driven by the
/// shipped suite. `declared` comes from `crate::compiler::cast::npc_casts` — the AUTHORED
/// ledger, the same authority `cast_dispatch` keys off — never from the emitted
/// bodies, so an emitter that skips an NPC still declares it and the skip is a
/// refusal. This is what makes "the exemplar search selected nobody" (the
/// gallery: eight clauses, zero proofs, green) unrepresentable rather than
/// merely fixed.
pub(super) fn cast_ladder_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    let casts = crate::compiler::cast::npc_casts(plan.campaign);
    crate::compiler::watch::Claim {
        mechanic: "cast-ladder",
        families: vec!["cast_".to_string()],
        declared: plan
            .npcs
            .iter()
            .filter(|n| casts.contains_key(&n.npc_id))
            .map(|n| n.safe.clone())
            .collect(),
    }
}

/// The claim the bark loop makes: one per ledger NPC (the family prefix carries
/// the NPC's own id, like `dialogue-mask`'s), declaring every bark SCENE of
/// that NPC — so a pool the loop skipped is a `DW0811` breach, not a smaller
/// green.
pub(super) fn cast_bark_watch_claims(plan: &Plan) -> Vec<crate::compiler::watch::Claim> {
    use crate::compiler::cast::SceneAction;
    let casts = crate::compiler::cast::npc_casts(plan.campaign);
    plan.npcs
        .iter()
        .filter_map(|npc| {
            let cast = casts.get(&npc.npc_id)?;
            let declared: BTreeSet<String> = cast
                .scenes
                .iter()
                .filter(|s| matches!(s.action, SceneAction::Barks(_)))
                .map(|s| s.index.to_string())
                .collect();
            if declared.is_empty() {
                return None;
            }
            Some(crate::compiler::watch::Claim {
                mechanic: "cast-bark",
                families: vec![format!("bark_{}_", npc.safe)],
                declared,
            })
        })
        .collect()
}
