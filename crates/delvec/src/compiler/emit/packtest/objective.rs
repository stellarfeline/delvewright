use super::*;

/// The AND-joins of a campaign: every objective with **two or more** `after`
/// prerequisites, with its quest and arms, in deterministic content order.
/// `after: [obj/a, obj/b]` is the DSL's AND primitive (spec-0018 adds no new
/// stage-5 syntax), and under party progression it is exactly the shape two
/// players split between two rooms.
fn and_joins(c: &delvewright_dsl::Campaign) -> Vec<(&str, &Objective)> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            if o.after().len() >= 2 {
                out.push((q.id.as_str(), o));
            }
        }
    }
    out
}

/// The party-size cap: a delve is played by one party of 1–4 (CLAUDE.md).
const MAX_PARTY: usize = 4;

/// Generated **division-of-labour** PackTests (spec-0018), one per AND-join.
///
/// The claim under test is the whole point of party progression: `n` DIFFERENT
/// players each complete exactly one arm of an `after` AND-join, and the
/// successor opens **for the party**. A single-dummy test cannot make that claim
/// — it would prove only that one player can do everything in sequence, which was
/// already true before the party holder existed. So each template spawns the
/// extra members itself with PackTest's `/dummy <name> spawn` (the framework
/// `# @dummy` supplies member 1 as `@s`) and drives one arm per member.
///
/// Three assertions, in order, and the middle one is the load-bearing negative:
///
/// 1. with no arm done, the join's real emitted guard is **not** satisfied;
/// 2. after member 1's arm alone it is **still** not satisfied (the AND is a real
///    AND — the successor does not leak open on one arm);
/// 3. after every member's arm it **is** satisfied, and the LAST member — never
///    the one who cleared the first arm — completes the join, proving each member
///    sees and can consume the successor state.
///
/// Batch model: own members (spawned and removed by this template alone,
/// under names no other template uses), own scratch holder (`#pj_<obj>`), own
/// init (every party score it reads is actively baselined), and no `await` — the
/// whole body is one atomic tick, so no sibling can interleave inside it.
///
/// `n` is the arm count, raised to `world.min_players` when the campaign declares
/// a bigger mandatory party and capped at [`MAX_PARTY`]; arms are handed out
/// round-robin, so a join with more arms than members gives someone two.
pub(super) fn emit_party_join_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let party = plan::PARTY;
    let min_players = plan::min_players(c) as usize;

    for (ji, (qid, join)) in and_joins(c).into_iter().enumerate() {
        let jid = join.id().as_str();
        let jsafe = plan::safe_local(jid);
        let arms: Vec<&str> = join.after().iter().map(|a| a.as_str()).collect();
        let n = arms.len().max(min_players).min(MAX_PARTY);
        // Member selectors. Member 1 is the framework dummy (`@s` — the binding
        // survives teleports and can never resolve to a neighbour's dummy);
        // members 2..n are spawned here under this template's own names.
        let name = |m: usize| format!("dwj{ji}p{m}");
        let member = |m: usize| {
            if m == 0 {
                "@s".to_string()
            } else {
                format!("@a[name={},limit=1]", name(m))
            }
        };
        let scratch = format!("#pj_{jsafe}");

        let mut b = packtest_header(&format!(
            "{}: AND-join `{jid}` divides across {n} players (spec-0018)",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        for m in 1..n {
            b.push(format!("dummy {} spawn", name(m)));
        }

        // --- own init: baseline every party score this join's guard reads ------
        let mut baseline: Vec<String> = Vec::new();
        let push_obj_baseline = |baseline: &mut Vec<String>, quest: &str, o: &Objective| {
            baseline.push(format!(
                "scoreboard players set {party} {} 1",
                quest_active_score(quest)
            ));
            for f in o.requires_flags() {
                baseline.push(format!(
                    "scoreboard players set {party} {} 1",
                    plan::flag_score(f.as_str())
                ));
            }
            for f in o.forbids_flags() {
                baseline.push(format!(
                    "scoreboard players set {party} {} 0",
                    plan::flag_score(f.as_str())
                ));
            }
        };
        // The join itself: quest active + its flag gates, its own score cleared.
        // Its `after` arms are deliberately NOT set — they are what the party
        // is about to earn.
        push_obj_baseline(&mut baseline, qid, join);
        baseline.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(jid)
        ));
        // Each arm: its own prerequisites satisfied, its own score cleared.
        for arm in &arms {
            let Some((aq, ao)) =
                objective_quest(c, arm).and_then(|(q, _)| find_objective(c, arm).map(|o| (q, o)))
            else {
                continue;
            };
            push_obj_baseline(&mut baseline, aq, ao);
            for prereq in ao.after() {
                baseline.push(format!(
                    "scoreboard players set {party} {} 1",
                    obj_score(prereq.as_str())
                ));
            }
            baseline.push(format!(
                "scoreboard players set {party} {} 0",
                obj_score(arm)
            ));
        }
        // Order-preserving dedup: sibling arms share a quest, so their
        // quest-active baselines coincide.
        let mut seen: BTreeSet<String> = BTreeSet::new();
        b.extend(baseline.into_iter().filter(|l| seen.insert(l.clone())));

        // The join's REAL emitted activation guard, materialized as a score so a
        // PackTest can assert it. Not a restatement: `pending_guard` is the very
        // function the `tick` driver uses.
        let guard = pending_guard(plan, join, &quest_active_score(qid));
        let probe = |b: &mut Vec<String>, expect: u32| {
            b.push(format!("scoreboard players set {scratch} dw.sys 0"));
            b.push(format!(
                "execute{guard} run scoreboard players set {scratch} dw.sys 1"
            ));
            b.push(format!("assert score {scratch} dw.sys matches {expect}"));
        };

        // 1. no arm done -> the join is shut.
        probe(&mut b, 0);

        // 2/3. each member clears exactly one arm (round-robin), and the party
        // score advances on THEIR action.
        for (k, arm) in arms.iter().enumerate() {
            b.push(format!(
                "execute as {} run function {ns}:complete_{}",
                member(k % n),
                safe_obj_fn(arm)
            ));
            b.push(format!("assert score {party} {} matches 1", obj_score(arm)));
            // After the FIRST arm (and while others remain) the join must still
            // be shut — the negative half that makes this an AND, not an OR.
            if k == 0 && arms.len() > 1 {
                probe(&mut b, 0);
            }
        }
        probe(&mut b, 1);

        // The successor is the PARTY's: the LAST member completes it, never the
        // one who cleared the first arm.
        b.push(format!(
            "execute as {} run function {ns}:complete_{}",
            member((arms.len() - 1) % n),
            safe_obj_fn(jid)
        ));
        b.push(format!("assert score {party} {} matches 1", obj_score(jid)));

        // No residue: the members this template spawned leave with it.
        for m in 1..n {
            b.push(format!("dummy {} leave", name(m)));
        }
        out.insert(
            format!("packtest-datapack/data/{ns}/test/party_join_{jsafe}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// The stage-5 objective with this id, across every quest.
fn find_objective<'a>(c: &'a delvewright_dsl::Campaign, id: &str) -> Option<&'a Objective> {
    c.quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .find(|o| o.id().as_str() == id)
}

/// What one objective's `activate_o_<id>` is supposed to MAKE EXIST, so a
/// template can assert it appeared.
///
/// Derived from [`activation_commands`] — the same function that writes the
/// body — rather than from a second hand-rolled table of objective kinds. A
/// table would be the very shape this check exists to catch: a walk over the
/// kinds someone remembered, silently missing the one added next.
enum ActivationFixture {
    /// N entities carrying the objective's own tag.
    Entities { tag: String, count: usize },
    /// The objective's stack in a container slot: the collect path summons
    /// nothing and fills a block instead.
    ContainerSlot { pos: [i32; 3], item: String },
    /// The objective's prop block alone (spec-0093 §6.5): a block vanilla
    /// reports the use of summons nothing — the block is the whole affordance.
    Block { pos: [i32; 3], block: String },
}

fn activation_fixture(cmds: &[String]) -> Option<ActivationFixture> {
    let summons = cmds.iter().filter(|c| c.starts_with("summon ")).count();
    if summons > 0 {
        // Every entity `activation_commands` summons carries the objective's own
        // tag (`dw_i_<obj>` / `dw_r_<obj>`), which is what `completion_cleanup`
        // kills them by. Read it off the first summon rather than recomputing it
        // from the objective kind.
        let tag = cmds
            .iter()
            .find(|c| c.starts_with("summon "))
            .and_then(|c| {
                c.split('"')
                    .find(|s| s.starts_with("dw_i_") || s.starts_with("dw_r_"))
            })?
            .to_string();
        return Some(ActivationFixture::Entities {
            tag,
            count: summons,
        });
    }
    // `setblock <x> <y> <z> <block>` and nothing else: a block-bound interact.
    if let [one] = cmds
        && let Some(rest) = one.strip_prefix("setblock ")
    {
        let f: Vec<&str> = rest.splitn(4, ' ').collect();
        if let [x, y, z, block] = f.as_slice() {
            let pos = [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?];
            return Some(ActivationFixture::Block {
                pos,
                block: crate::compiler::pressable::block_id(block).to_string(),
            });
        }
    }
    // `item replace block <x> <y> <z> container.<n> with <item>[…] <count>`
    let fill = cmds.iter().find(|c| c.starts_with("item replace block "))?;
    let f: Vec<&str> = fill.split_whitespace().collect();
    let pos = [
        f.get(3)?.parse().ok()?,
        f.get(4)?.parse().ok()?,
        f.get(5)?.parse().ok()?,
    ];
    // `item replace block <x> <y> <z> <slot> with <stack> [<count>]` — the stack
    // is the field AFTER the `with` keyword, which is read by name rather than
    // by a counted offset. The item id alone, without its component tail: `if
    // items block` takes an item predicate, and the id is the part of the stack
    // the activation is being judged on.
    let stack = f.iter().skip_while(|w| **w != "with").nth(1)?;
    let item = stack
        .split_once('[')
        .map(|(id, _)| id)
        .unwrap_or(stack)
        .to_string();
    Some(ActivationFixture::ContainerSlot { pos, item })
}

/// Every objective's own activation body really materialises THAT objective's
/// affordance.
///
/// `activate_o_<id>` is per-objective code: it summons the objective's own
/// hitbox or marker at the objective's own cell, or fills the objective's own
/// container with the objective's own stack. The gallery drove two of five —
/// both interact-shaped — so all three `reach` markers shipped with no runtime
/// proof that the thing a player is told to walk to ever appears.
///
/// Residue-free by construction, which is what lets it run beside the templates
/// that already drive an activation: it never zeroes the `#act_<id>` latch (that
/// would tell the campaign tick an objective is live when it is not) and it
/// takes a BEFORE count, so it neither assumes a fresh world nor destroys a
/// sibling's fixtures — it removes exactly the entities it caused.
pub(super) fn emit_objective_activation_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let title = artifact_title(plan.campaign);
    for q in &plan.campaign.quests.content.quests {
        let area = plan.quest_area(q.id.as_str()).unwrap_or("");
        for o in &q.objectives {
            let oid = o.id().as_str();
            let cmds = activation_commands(plan, area, o);
            let Some(fixture) = activation_fixture(&cmds) else {
                continue;
            };
            let safe = plan::safe_local(oid);
            let mut b = packtest_header(&format!(
                "{title}: activating objective `{oid}` places its own affordance"
            ));
            b.push(format!("function {ns}:setup"));
            match &fixture {
                ActivationFixture::Entities { tag, count } => {
                    b.push(format!(
                        "execute store result score #fx0_{safe} dw.sys if entity @e[tag={tag}]"
                    ));
                    b.push(format!("function {ns}:activate_o_{safe}"));
                    b.push(format!(
                        "execute store result score #fx1_{safe} dw.sys if entity @e[tag={tag}]"
                    ));
                    b.push(format!(
                        "scoreboard players operation #fx1_{safe} dw.sys -= #fx0_{safe} dw.sys"
                    ));
                    b.push(format!("assert score #fx1_{safe} dw.sys matches {count}"));
                    // Remove exactly what this template caused, and nothing else.
                    b.push(format!("kill @e[tag={tag},limit={count}]"));
                }
                ActivationFixture::ContainerSlot { pos, item } => {
                    let (x, y, z) = (pos[0], pos[1], pos[2]);
                    // Empty the objective's own slot first, so the after-read is a
                    // fact about this activation rather than about whatever stood
                    // in the container already. The activation refills it, which
                    // is also the cleanup.
                    b.push(format!(
                        "item replace block {x} {y} {z} container.0 with minecraft:air"
                    ));
                    b.push(format!(
                        "execute store success score #fx0_{safe} dw.sys if items block {x} {y} {z} \
                         container.0 {item}"
                    ));
                    b.push(format!("assert score #fx0_{safe} dw.sys matches 0"));
                    b.push(format!("function {ns}:activate_o_{safe}"));
                    b.push(format!(
                        "execute store success score #fx1_{safe} dw.sys if items block {x} {y} {z} \
                         container.0 {item}"
                    ));
                    b.push(format!("assert score #fx1_{safe} dw.sys matches 1"));
                }
                ActivationFixture::Block { pos, block } => {
                    let (x, y, z) = (pos[0], pos[1], pos[2]);
                    // Clear the cell first, so the after-read is a fact about this
                    // activation; the activation puts the block back, which is
                    // also the cleanup.
                    b.push(format!("setblock {x} {y} {z} minecraft:air"));
                    b.push(format!(
                        "execute store success score #fx0_{safe} dw.sys if block {x} {y} {z} {block}"
                    ));
                    b.push(format!("assert score #fx0_{safe} dw.sys matches 0"));
                    b.push(format!("function {ns}:activate_o_{safe}"));
                    b.push(format!(
                        "execute store success score #fx1_{safe} dw.sys if block {x} {y} {z} {block}"
                    ));
                    b.push(format!("assert score #fx1_{safe} dw.sys matches 1"));
                }
            }
            out.insert(
                format!("packtest-datapack/data/{ns}/test/obj_activate_{safe}.mcfunction"),
                lines(&b).into_bytes(),
            );
        }
    }
}

pub(super) fn objective_activation_watch_claim(plan: &Plan) -> crate::compiler::watch::Claim {
    crate::compiler::watch::Claim {
        mechanic: "objective-activation",
        families: vec!["activate_o_".to_string()],
        declared: plan
            .campaign
            .quests
            .content
            .quests
            .iter()
            .flat_map(|q| q.objectives.iter())
            .map(|o| plan::safe_local(o.id().as_str()))
            .collect(),
    }
}

/// Emit a per-verb mechanism PackTest for the first `kill` / `collect` /
/// `interact` objective, plus a flag-gate test for the first flag-gated
/// collect/interact objective and a forbid-gate test for the first
/// `forbids_flags`-gated one (v0.6 negative gate).
pub(super) fn emit_verb_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;

    let mut write = |name: &str, body: Vec<String>| {
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{name}.mcfunction"),
            lines(&body).into_bytes(),
        );
    };

    // Collect (quest, objective) pairs by verb, in declared order.
    let mut first_kill = None;
    // Prefer a kill whose wave contains an armed mob so the armed-equipment assert
    // (M2 round-2 fix 1) is actually exercised — the equipment bug hid for a whole
    // milestone precisely because nothing looked. Falls back to the first kill.
    let mut first_armed_kill = None;
    let mut first_collect = None;
    // The first `collect` that adopts a prefab container (DSL v0.8).
    let mut first_collect_adopted = None;
    let mut first_interact = None;
    // The first `interact` that actually gates on an item — the subject of the
    // held-vs-carried test below. Distinct from `first_interact`, which may be
    // ungated and would make that test vacuous.
    let mut first_interact_item = None;
    let mut first_flag_gated = None;
    let mut first_forbid_gated = None;
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            let qid = q.id.as_str();
            match o {
                Objective::Kill { wave, .. } => {
                    if first_kill.is_none() {
                        first_kill = Some((qid, o));
                    }
                    if first_armed_kill.is_none()
                        && plan::wave_of(c, wave.as_str()).is_some_and(|w| {
                            w.mobs.iter().any(|m| {
                                effective_mainhand(&m.entity, m.equipment.as_ref()).is_some()
                            })
                        })
                    {
                        first_armed_kill = Some((qid, o));
                    }
                }
                Objective::Collect { .. } if first_collect.is_none() => {
                    first_collect = Some((qid, o))
                }
                Objective::Interact { .. } => {
                    if first_interact.is_none() {
                        first_interact = Some((qid, o));
                    }
                    if first_interact_item.is_none()
                        && matches!(
                            o,
                            Objective::Interact {
                                requires_item: Some(_),
                                ..
                            }
                        )
                    {
                        first_interact_item = Some((qid, o));
                    }
                }
                _ => {}
            }
            // v0.8: the first `collect` that ADOPTS a prefab container.
            // Distinct from `first_collect`, which may keep the compiler-placed
            // chest and would make the adoption assertions vacuous.
            if first_collect_adopted.is_none() && o.collect_container().is_some() {
                first_collect_adopted = Some((qid, o));
            }
            if first_flag_gated.is_none()
                && !o.requires_flags().is_empty()
                && matches!(o, Objective::Collect { .. } | Objective::Interact { .. })
            {
                first_flag_gated = Some((qid, o));
            }
            if first_forbid_gated.is_none()
                && !o.forbids_flags().is_empty()
                && matches!(o, Objective::Collect { .. } | Objective::Interact { .. })
            {
                first_forbid_gated = Some((qid, o));
            }
        }
    }
    let first_kill = first_armed_kill.or(first_kill);

    // kill: spawn the wave, drain the countdown via the kill reward, tick,
    // assert the objective completed.
    if let Some((qid, o)) = first_kill
        && let Objective::Kill { id, wave, .. } = o
        && let Some(w) = plan::wave_of(c, wave.as_str())
    {
        let total = plan::wave_total(w);
        let ws = plan::safe_local(wave.as_str());
        let (pin, sel) = pin_dummy("dw_t_vkil");
        let mut b = packtest_header(&format!(
            "{}: kill wave `{wave}` -> countdown -> complete",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive the whole chain
        // on it alone; actively zero the asserted objective first.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        // Clear the wave tag before the fresh spawn — a sibling test may have
        // already fired this spawn-wave (`spawn_<wave>` is unguarded).
        b.push(format!("kill @e[tag={}]", plan::wave_tag(wave.as_str())));
        b.push(format!("function {ns}:spawn_{ws}"));
        b.push(format!(
            "assert score {} {} matches {total}",
            plan::wave_counter(wave.as_str()),
            plan::WAVE_OBJECTIVE
        ));
        // The armed mob really holds its weapon (M2 round-2 fix 1). `HandItems`
        // failed silently for a whole milestone because no test looked; this
        // exercises the vanilla `execute if items entity … weapon.mainhand …`
        // condition (1.21.11 `minecraft:item_slots` + `minecraft:item_predicate`)
        // and bridges the result to `assert score` — using only PackTest commands
        // known-good on the validation server, not a newer `assert items`.
        // The asserted item is the mob's *effective* main hand — an author's
        // `equipment.main_hand` override when present, the default table
        // otherwise — so the assertion always describes the summon this same
        // compiler emitted (see `effective_mainhand`).
        if let Some((mob, item)) = w
            .mobs
            .iter()
            .find_map(|m| effective_mainhand(&m.entity, m.equipment.as_ref()).map(|it| (m, it)))
        {
            b.push("scoreboard players set #armed_vkil dw.sys 0".to_string());
            b.push(format!(
                "execute if items entity @e[tag={},type={},limit=1] weapon.mainhand {item} \
                 run scoreboard players set #armed_vkil dw.sys 1",
                plan::wave_tag(wave.as_str()),
                mob.entity,
            ));
            b.push("assert score #armed_vkil dw.sys matches 1".to_string());
        }
        b.push(format!("kill @e[tag={}]", plan::wave_tag(wave.as_str())));
        for _ in 0..total {
            b.push(format!("execute as {sel} run function {ns}:k_reward_{ws}"));
        }
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        write("verb_kill", b);
    }

    // kill, with NOBODY credited for the deaths. The countdown used to be a tally
    // of `minecraft:player_killed_entity` grants, and vanilla has no trigger for
    // "this entity died" — so a wave mob that fell, burned, drowned, walked into a
    // lethal volume or was cut down by another mob left the countdown stuck above
    // zero and the objective open forever, along with everything gated behind it.
    // `/kill` is the cheapest death vanilla has that credits no player, and it is
    // the same shape as every one of those: health to zero, no advancement.
    //
    // Deliberately NOT written as a variant of `verb_kill`: that one drives
    // `k_reward_<wave>` by hand, so it can only ever prove the tally path. This
    // one never touches the reward, which is exactly what makes it able to fail.
    if let Some((qid, o)) = first_kill
        && let Objective::Kill { id, wave, .. } = o
        && let Some(w) = plan::wave_of(c, wave.as_str())
    {
        let total = plan::wave_total(w);
        let ws = plan::safe_local(wave.as_str());
        let tag = plan::wave_tag(wave.as_str());
        let counter = plan::wave_counter(wave.as_str());
        let wobj = plan::WAVE_OBJECTIVE;
        let (pin, sel) = pin_dummy("dw_t_vkun");
        let mut b = packtest_header(&format!(
            "{}: wave `{wave}` clears when its bodies die with no player credited",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        // Own init: `spawn_<wave>` is unguarded and a sibling may already have
        // fired it, so clear before spawning — the spawn is what writes the total.
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:spawn_{ws}"));
        b.push(format!("assert score {counter} {wobj} matches {total}"));
        // Every body dies, and no player is credited with any of it.
        b.push(format!("kill @e[tag={tag}]"));
        b.push(format!("function {ns}:tick"));
        // The outcome is the objective, and only the countdown reaching zero can
        // produce it — so this one assertion carries the whole mechanism,
        // including that corpses still in their death animation are not counted.
        // The countdown itself is deliberately NOT asserted here: after the drive
        // it is an outcome `spawn_<wave>` also writes, and `DW0807` would then
        // (correctly) make every gate on the quest that spawns this wave this
        // template's business.
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        write("verb_kill_uncredited", b);
    }

    // collect: satisfy guards + hold the item, run the collect reward, assert.
    if let Some((qid, o)) = first_collect
        && let Objective::Collect { id, .. } = o
    {
        let (pin, sel) = pin_dummy("dw_t_vcol");
        let mut b = packtest_header(&format!(
            "{}: collect -> reward completes objective",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it
        // alone; actively zero the asserted objective first.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        b.push(format!(
            "execute as {sel} run function {ns}:c_reward_{}",
            plan::safe_local(id.as_str())
        ));
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        write("verb_collect", b);
    }

    // interact: hold the required item, fire the trigger, tick, assert.
    if let Some((qid, o)) = first_interact
        && let Objective::Interact { id, .. } = o
    {
        let (pin, sel) = pin_dummy("dw_t_vint");
        let mut b = packtest_header(&format!(
            "{}: interact trigger + item -> complete",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it
        // alone — the old `@a`-wide preamble was the round-5 flag leak that
        // poisoned `verb_flag_gate`'s withheld phase.
        b.push(pin);
        b.push(format!(
            "scoreboard players set {} {} 0",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        // spec-0093 §6.5: a block a hand presses has no hitbox and no chat
        // command; its press is the advancement, so the test GRANTS it — which
        // runs the reward as the dummy — and the reward is what sets the score.
        if crate::compiler::pressable::interact_block(o).is_some() {
            b.push(format!(
                "advancement grant {sel} only {ns}:i_{}",
                plan::safe_local(id.as_str())
            ));
        } else {
            b.push(format!(
                "scoreboard players set {sel} {} 1",
                plan::interact_trigger(id.as_str())
            ));
        }
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {} {} matches 1",
            plan::PARTY,
            obj_score(id.as_str())
        ));
        write("verb_interact", b);

        // A click that lands before its quest is armed is DISCARDED, and a real
        // click afterwards still works.
        //
        // This is the runtime half of the arming invariant. The compile-time half
        // (`tests/tick_arming.rs`) pins that the arming quest's lines precede the
        // adjudication, so a pending click can never be lost to same-tick
        // ordering. What a live server has to show is the other half: the
        // unconditional reset really does SPEND a premature click rather than
        // bank it — because a banked click would auto-complete the objective the
        // instant the quest armed, with nobody having clicked anything.
        let (pin, sel) = pin_dummy("dw_t_varm");
        let trigger = plan::interact_trigger(id.as_str());
        let obj = obj_score(id.as_str());
        let qa = quest_active_score(qid);
        let party = plan::PARTY;
        let mut b = packtest_header(&format!(
            "{}: a click before the quest is armed is spent, not banked",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        // Baseline: objective open, quest NOT armed, and the preamble's other
        // guards satisfied so the arming flag is the only thing standing in the
        // way. The preamble sets the quest active, so it is cleared after it.
        b.push(format!("scoreboard players set {party} {obj} 0"));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        b.push(format!("scoreboard players set {party} {qa} 0"));

        // --- the premature click: no completion, and no banked trigger ---
        b.push(format!("scoreboard players set {sel} {trigger} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!("assert score {party} {obj} matches 0"));
        b.push(format!(
            "execute store result score #varm_bank dw.sys if score {sel} {trigger} matches 1.."
        ));
        b.push("assert score #varm_bank dw.sys matches 0".to_string());

        // --- arming alone must not complete it: the click is genuinely gone ---
        b.push(format!("scoreboard players set {party} {qa} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!("assert score {party} {obj} matches 0",));

        // --- and a real click, now that the quest is armed, still completes ---
        b.push(format!("scoreboard players set {sel} {trigger} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!("assert score {party} {obj} matches 1"));
        write("verb_interact_arming", b);
    }

    // interact + `requires_item`: HELD, not merely carried. Two phases on one
    // dummy, one tick each:
    //
    //   A. the item is in the pack, the hand is empty  -> must NOT complete
    //   B. the same item is in the main hand           -> completes
    //
    // Phase A first proves the item really is carried (`if items entity @s
    // container.*` bridged to a score) — without that, "did not complete" would be
    // indistinguishable from "had no item at all", i.e. a vacuous test that the old
    // inventory-wide gate would also have passed.
    //
    // What this template deliberately does NOT assert is the `missing_item_hint`
    // narration: PackTest can observe scores, blocks and entities, but a `tellraw`
    // leaves no game state to assert against — there is no chat-log primitive, and
    // inventing a "did it narrate" scoreboard side-channel inside the emitted tick
    // would be a hack the player pays for (no-hack doctrine). The hint's emission is
    // proven instead by an exact-line assertion in `crates/delvec/tests/v07_*.rs`,
    // and its in-game appearance by the live tier. The mechanism it rides on — the
    // main-hand gate itself — is what this template proves.
    if let Some((qid, o)) = first_interact_item
        && let Objective::Interact {
            id,
            requires_item: Some(it),
            ..
        } = o
    {
        let (pin, sel) = pin_dummy("dw_t_vheld");
        let trigger = plan::interact_trigger(id.as_str());
        let party = plan::PARTY;
        let carried = "#carried_vheld";
        let mut b = packtest_header(&format!(
            "{}: `requires_item` is HELD — carried is not enough",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id.as_str())
        ));
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        // --- phase A: carried, not held ---
        b.push(format!(
            "item replace entity {sel} weapon.mainhand with minecraft:air"
        ));
        b.push(format!("item replace entity {sel} inventory.0 with {it}"));
        b.push(format!("scoreboard players set {carried} dw.sys 0"));
        b.push(format!(
            "execute as {sel} if items entity @s container.* {it} run scoreboard players set {carried} dw.sys 1"
        ));
        b.push(format!("assert score {carried} dw.sys matches 1"));
        b.push(format!("scoreboard players set {sel} {trigger} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {party} {} matches 0",
            obj_score(id.as_str())
        ));
        // --- phase B: the same item, now presented ---
        b.push(format!(
            "item replace entity {sel} inventory.0 with minecraft:air"
        ));
        b.push(format!(
            "item replace entity {sel} weapon.mainhand with {it}"
        ));
        b.push(format!("scoreboard players set {sel} {trigger} 1"));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {party} {} matches 1",
            obj_score(id.as_str())
        ));
        write("verb_interact_held", b);
    }

    // flag gate: without the flag the objective must NOT complete; with it, it
    // does. The dummy is pinned and the withheld flags actively cleared (see
    // `pin_dummy` / `packtest_preamble`): a sibling template that satisfies the
    // same gated objective (`verb_interact`) sets the flag on `@a` — every
    // dummy in the batch — so this test must establish "flag absent" itself,
    // on its own dummy, rather than assume a fresh player.
    if let Some((qid, o)) = first_flag_gated {
        let id = o.id().as_str();
        let (pin, sel) = pin_dummy("dw_flagtest");
        let driver = |b: &mut Vec<String>| match o {
            Objective::Collect { .. } => b.push(format!(
                "execute as {sel} run function {ns}:c_reward_{}",
                plan::safe_local(id)
            )),
            Objective::Interact { .. } => {
                b.push(format!(
                    "scoreboard players set {sel} {} 1",
                    plan::interact_trigger(id)
                ));
                b.push(format!("function {ns}:tick"));
            }
            _ => {}
        };
        let mut b = packtest_header(&format!(
            "{}: requires_flags gates objective `{id}`",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        let party = plan::PARTY;
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id)
        ));
        b.extend(packtest_preamble(plan, qid, o, false, &sel)); // flags withheld (cleared)
        // `with_flags: false` shuts EVERY gate the objective has, the numeric one
        // included — which is right for a preamble and wrong for this test. This
        // template's subject is `requires_flags`, so the flag must be the only
        // variable: with the numeric gate also shut, the withheld assert passes
        // because TWO gates are closed (it would pass with the flag logic
        // deleted), and the released phase — which reopens only the flags — can
        // never pass at all. An objective carrying both gates therefore emitted a
        // test that could not go green, and nothing had ever written both on one
        // objective until the gallery did.
        b.extend(state_drive_lines(plan, o.requires_state(), true));
        driver(&mut b);
        b.push(format!("assert score {party} {} matches 0", obj_score(id)));
        for f in o.requires_flags() {
            b.push(format!(
                "scoreboard players set {party} {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {party} {} matches 1", obj_score(id)));
        write("verb_flag_gate", b);
    }

    // forbid gate (v0.6 negative gate): with a forbidden flag SET the objective
    // must NOT complete; with it cleared, it does. The mirror image of
    // `verb_flag_gate`, phases reversed (suppress first, then release) so both
    // truth-table rows of the negative gate are exercised on one dummy.
    if let Some((qid, o)) = first_forbid_gated {
        let id = o.id().as_str();
        let (pin, sel) = pin_dummy("dw_fbdtest");
        let driver = |b: &mut Vec<String>| match o {
            Objective::Collect { .. } => b.push(format!(
                "execute as {sel} run function {ns}:c_reward_{}",
                plan::safe_local(id)
            )),
            Objective::Interact { .. } => {
                b.push(format!(
                    "scoreboard players set {sel} {} 1",
                    plan::interact_trigger(id)
                ));
                b.push(format!("function {ns}:tick"));
            }
            _ => {}
        };
        let mut b = packtest_header(&format!(
            "{}: forbids_flags suppresses objective `{id}`",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin.clone());
        let party = plan::PARTY;
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id)
        ));
        // Preamble satisfies quest/after/requires and CLEARS forbids; then set
        // the forbidden flags to prove suppression.
        b.extend(packtest_preamble(plan, qid, o, true, &sel));
        for f in o.forbids_flags() {
            b.push(format!(
                "scoreboard players set {party} {} 1",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {party} {} matches 0", obj_score(id)));
        for f in o.forbids_flags() {
            b.push(format!(
                "scoreboard players set {party} {} 0",
                plan::flag_score(f.as_str())
            ));
        }
        driver(&mut b);
        b.push(format!("assert score {party} {} matches 1", obj_score(id)));
        write("verb_forbid_gate", b);
    }

    // gap 9: every NPC body actually summoned. The bot drives talk-to via a
    // `/trigger` chat command, so a failed summon (e.g. an invalid `base_entity`)
    // would still pass the ladder with no NPC in the world — a false green. This
    // asserts each NPC's body resolves to EXACTLY one entity. It summons
    // deterministically, independent of the async placement/tick loop: disarm the
    // tick placer (`#placed`) and clear any body/hitbox a prior boot or test left
    // at the same absolute coords, then run `setup_finish` once (it summons at the
    // chunks `setup` force-loads; no templates needed). v0.3-gated so v0.2
    // campaigns (hello-world has an NPC) keep byte-identical packtest output.
    if !plan.npcs.is_empty() {
        let mut b = packtest_header(&format!(
            "{}: every NPC summon resolves to exactly one entity",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push("scoreboard players set #placed dw.sys 1".to_string());
        for npc in &plan.npcs {
            b.push(format!("kill @e[tag={}]", npc.tag));
        }
        b.push(format!("function {ns}:setup_finish"));
        // A `deferred` NPC (DSL v0.6) is deliberately absent after `setup_finish` —
        // it enters via `spawn-npc`. Fire its entrance here, so this test proves the
        // deferred path summons exactly the same one body + one hitbox.
        for npc in &plan.npcs {
            if npc_is_deferred(c, &npc.npc_id) {
                b.push(format!("function {ns}:{}", spawn_npc_fn(&npc.npc_id)));
            }
        }
        for npc in &plan.npcs {
            // The NPC body carries BOTH `dw_npc` and its unique id tag; the separate
            // interaction hitbox carries only the id tag — so `dw_npc` + id tag
            // selects exactly the body. A failed body summon leaves zero.
            b.push(format!(
                "execute store result score #npc_{} dw.sys if entity @e[tag=dw_npc,tag={}]",
                npc.safe, npc.tag
            ));
            b.push(format!("assert score #npc_{} dw.sys matches 1", npc.safe));
        }
        write("npc_summons", b);
    }

    // gap 13: a collect item taken BEFORE the objective activates must still
    // complete it at activation, with no further inventory churn. Reproduces the
    // stall: pick the item up while the quest is inactive (arming and stranding the
    // re-arming `inventory_changed` advancement), THEN activate and tick once with
    // no further pickup — the per-tick held check must complete the objective.
    if let Some((qid, o)) = first_collect
        && let Objective::Collect {
            id, item, count, ..
        } = o
    {
        let (pin, sel) = pin_dummy("dw_t_cpre");
        let mut b = packtest_header(&format!(
            "{}: collect completes for an item held before activation",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        // Pin this test's own dummy (see `pin_dummy`) and drive/assert on it alone.
        b.push(pin);
        let party = plan::PARTY;
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id.as_str())
        ));
        // Take the item while the objective is INACTIVE (the pre-activation pickup).
        b.push(format!("give {sel} {item} {count}"));
        // Activate WITHOUT re-giving: `packtest_preamble` would re-give the item,
        // masking the bug by producing a fresh `inventory_changed`. Only the GIVE
        // is unwanted, so take the guard half whole (`packtest_guards`) rather
        // than re-deriving it — this site used to hand-roll quest-active, `after`
        // and `requires_flags` and stopped there, silently omitting the v0.6
        // negative axis and the v0.10 numeric one, both of which this objective's
        // own tick line reads.
        b.extend(packtest_guards(plan, qid, o, true));
        // One tick's held check completes it — no inventory_changed event occurs.
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {party} {} matches 1",
            obj_score(id.as_str())
        ));
        write("collect_preheld", b);
    }

    // v0.8 container adoption: the
    // objective fills the barrel the PREFAB placed, pads it so it reads full, and
    // still completes when what the player carries is the NAMED stack.
    //
    // Three things a compile-time test cannot reach, all of them silent failures
    // on a live server: `item replace block … container.<n>` against the adopted
    // cell has to actually land (it fails without output on a non-container —
    // `DW0438` proves one is there, not that the fill took); the padding has to
    // occupy the slots after it rather than overwrite slot 0; and the custom-name
    // component must not change what the adjudication sees, because the
    // completion advancement and the per-tick held check both match on ITEM ID
    // and a component that quietly excluded the stack would leave the objective
    // uncompletable with the item sitting in the player's hand.
    if let Some((qid, o)) = first_collect_adopted
        && let Objective::Collect {
            id,
            item,
            count,
            item_name,
            fill_count,
            ..
        } = o
        && let Some(fill) = plan
            .collect_fills
            .iter()
            .find(|f| f.objective_id == id.as_str())
    {
        let (pin, sel) = pin_dummy("dw_t_cadp");
        let cell = fill.cell;
        let party = plan::PARTY;
        let stack = format!(
            "{item}{} {count}",
            item_component_tail(item, item_name.as_deref())
        );
        let mut b = packtest_header(&format!(
            "{}: collect `{id}` fills the adopted container and completes on the named stack",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!(
            "scoreboard players set {party} {} 0",
            obj_score(id.as_str())
        ));
        // Open the activation gate by hand and WITHOUT the preamble's `give`: the
        // point of this template is which stack completes the objective, and the
        // plain item handed over first would complete it before the named one is
        // ever presented.
        b.extend(packtest_guards(plan, qid, o, true));
        // Empty the adopted container first — `setup` may have run for a sibling
        // template, and this objective's own activation is guarded once per world
        // by `#act_<obj>`, so the fill is not re-run on a second call. Clearing
        // makes the count assertion below a statement about THIS activation.
        for slot in 0..=*fill_count {
            b.push(format!(
                "item replace block {} {} {} container.{slot} with minecraft:air",
                cell[0], cell[1], cell[2]
            ));
        }
        b.push(format!(
            "function {ns}:activate_{}",
            safe_obj_fn(id.as_str())
        ));
        // The fill landed, in the right number of slots: `if items block` counts
        // matching items across the whole container, so the total is the stack
        // repeated once per filled slot. A dropped fill reads 0; padding that
        // overwrote slot 0 instead of following it reads one stack short.
        let total = count * (fill_count + 1);
        b.push(format!(
            "execute store result score #cadp dw.sys if items block {} {} {} container.* {item}",
            cell[0], cell[1], cell[2]
        ));
        b.push(format!("assert score #cadp dw.sys matches {total}"));
        // The player takes the stack the container actually holds — components and
        // all, the same text the fill emitted — and the objective completes.
        b.push(format!(
            "item replace entity {sel} inventory.0 with {stack}"
        ));
        b.push(format!("function {ns}:tick"));
        b.push(format!(
            "assert score {party} {} matches 1",
            obj_score(id.as_str())
        ));
        write("collect_container", b);
    }
}
