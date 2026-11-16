//! The party-division checks (spec-0018) and the one line they print: the
//! AND-joins bound to a `party_join_<obj>` template, the `carrier: "one"`
//! `give-item`s a two-player template judges, and the `min_players` division
//! proof.

use super::*;

/// Generated **one-carrier** PackTests (spec-0018), one per objective whose
/// completion bundle hands a `carrier: "one"` item at its top level with no
/// `when`/`in` envelope.
///
/// The claim: with two players present, the item reaches exactly one of them —
/// the one whose action completed the objective — and not the other. A
/// single-dummy test cannot make it: `give @a` and `give @s` are the same command
/// to a party of one. So the template spawns a second member, has the framework
/// dummy (`@s`) complete the objective through its real `complete_<obj>`, and
/// counts each member's copies with `clear <who> <item> 0`, which removes
/// nothing and returns the count.
///
/// Batch model: own member (spawned and removed here, under a name no other
/// template uses), own scratch holders (`#pc_<obj>_<i>_a` / `_b`), own init (both
/// members' copies are cleared before the drive), and no `await`.
pub(super) fn emit_carrier_one_packtests(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    for (k, (oid, gives)) in judged_carrier_gives(c).into_iter().enumerate() {
        let osafe = plan::safe_local(oid);
        let other = format!("dwc{k}p1");
        let other_sel = format!("@a[name={other},limit=1]");
        let mut b = packtest_header(&format!(
            "{}: `carrier: one` at `{oid}` reaches one of two players (spec-0018)",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("dummy {other} spawn"));
        // Own init: neither member holds a copy before the drive.
        let items: BTreeSet<&str> = gives.iter().map(|(item, _)| *item).collect();
        for item in &items {
            b.push(format!("clear @s {item}"));
            b.push(format!("clear {other_sel} {item}"));
        }
        b.push(format!(
            "execute as @s run function {ns}:complete_{}",
            safe_obj_fn(oid)
        ));
        for (i, item) in items.iter().enumerate() {
            let want: u32 = gives
                .iter()
                .filter(|(it, _)| it == item)
                .map(|(_, n)| *n)
                .sum();
            let a = format!("#pc_{osafe}_{i}_a");
            let o = format!("#pc_{osafe}_{i}_b");
            b.push(format!(
                "execute store result score {a} dw.sys run clear @s {item} 0"
            ));
            b.push(format!(
                "execute store result score {o} dw.sys run clear {other_sel} {item} 0"
            ));
            // The completing player holds every copy, the other member none.
            b.push(format!("assert score {a} dw.sys matches {want}"));
            b.push(format!("assert score {o} dw.sys matches 0"));
        }
        for item in &items {
            b.push(format!("clear @s {item}"));
        }
        b.push(format!("dummy {other} leave"));
        out.insert(carrier_template_path(ns, oid), lines(&b).into_bytes());
    }
}

fn carrier_template_path(ns: &str, oid: &str) -> String {
    format!(
        "packtest-datapack/data/{ns}/test/party_carrier_{}.mcfunction",
        plan::safe_local(oid)
    )
}

/// Whether a `give-item` is one the one-carrier template can judge: `carrier:
/// "one"`, with no `when` (the template would have to open a gate it does not
/// own) and no `in` (the completing player would have to stand in a box).
fn judgeable(e: &QuestEffect) -> bool {
    e.gives_to_one() && e.when.is_none() && e.within.is_none()
}

/// Per objective (content order), the `(item, count)` of every judgeable
/// `carrier: "one"` give at the top level of its completion bundle.
fn judged_carrier_gives(c: &delvewright_dsl::Campaign) -> Vec<(&str, Vec<(&str, u32)>)> {
    let mut out = Vec::new();
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            let oid = o.id().as_str();
            let gives: Vec<(&str, u32)> = objective_effects(c, oid)
                .into_iter()
                .filter(|e| judgeable(e))
                .filter_map(|e| match &e.verb {
                    Verb::GiveItem {
                        source: delvewright_dsl::GiveSource::Stack(s),
                        ..
                    } => Some((s.item.as_str(), s.count)),
                    _ => None,
                })
                .collect();
            if !gives.is_empty() {
                out.push((oid, gives));
            }
        }
    }
    out
}

/// What the party-division checks bound on one build, counted off the emitted
/// suite: a join or a give counts as bound only when its template is in `out`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PartyBinding {
    /// Stage-5 objectives in the campaign.
    pub objectives: usize,
    /// Of those, AND-joins (two or more `after` arms).
    pub and_joins: usize,
    /// AND-joins whose `party_join_<obj>` template the suite carries.
    pub and_joins_bound: usize,
    /// `party_join_*` templates in the suite.
    pub party_join_templates: usize,
    /// Every PackTest template in the suite.
    pub templates: usize,
    /// `give-item` effects at every effect root and nesting depth.
    pub gives: usize,
    /// Of those, `carrier: "one"`.
    pub carrier_one: usize,
    /// Of those, asserted by a one-carrier template the suite carries.
    pub carrier_one_judged: usize,
    /// `carrier: "one"` gives at an objective completion's top level that the
    /// template cannot judge: under a `when` or an `in` envelope.
    pub carrier_one_enveloped: usize,
    /// `carrier: "one"` gives anywhere else: a quest completion, a presser
    /// trigger, a nested bundle, any other root.
    pub carrier_one_elsewhere: usize,
    /// The declared `world.min_players`.
    pub min_players: usize,
    /// The analyzer's division for `min_players >= 2`: the join and each
    /// agent's arms. `None` for a party of one, which owes no division.
    pub division: Option<Result<crate::compiler::flow::Division, String>>,
}

impl PartyBinding {
    /// Whether every AND-join and every judgeable `carrier: "one"` give has the
    /// template it is owed.
    pub fn complete(&self) -> bool {
        self.and_joins_bound == self.and_joins
            && self.carrier_one_judged
                == self.carrier_one - self.carrier_one_enveloped - self.carrier_one_elsewhere
    }

    /// The one line a build prints about the party-division checks.
    pub fn line(&self) -> String {
        let division = match &self.division {
            None => format!(
                "`min_players: {}` owes no division (a party of one is the single-agent proof)",
                self.min_players
            ),
            Some(Ok(crate::compiler::flow::Division { join, agents })) => format!(
                "`min_players: {}` divided over `{join}` ({})",
                self.min_players,
                agents
                    .iter()
                    .enumerate()
                    .map(|(i, a)| format!("agent {}: {}", i + 1, a.join(", ")))
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Some(Err(why)) => format!("`min_players: {}` has no division: {why}", self.min_players),
        };
        format!(
            "party binding (spec-0018): {} AND-join(s) of {} objective(s), {} bound to a \
             `party_join` PackTest ({} `party_join` template(s) of {}); {} `carrier: one` \
             `give-item`(s) of {}, {} judged by a two-player PackTest (not judged: {} under a \
             `when`/`in` envelope, {} outside an objective's completion bundle); {division}",
            self.and_joins,
            self.objectives,
            self.and_joins_bound,
            self.party_join_templates,
            self.templates,
            self.carrier_one,
            self.gives,
            self.carrier_one_judged,
            self.carrier_one_enveloped,
            self.carrier_one_elsewhere,
        )
    }

    /// The ledger `validation/party-division.json` carries. `unbound` is set
    /// when a join or a judgeable give has no template.
    pub fn to_json(&self) -> serde_json::Value {
        let division = match &self.division {
            None => serde_json::Value::Null,
            Some(Ok(d)) => json!({ "join": d.join, "agents": d.agents }),
            Some(Err(why)) => json!({ "refused": why }),
        };
        let mut doc = json!({
            "objectives": self.objectives,
            "and_joins": self.and_joins,
            "and_joins_bound": self.and_joins_bound,
            "party_join_templates": self.party_join_templates,
            "templates": self.templates,
            "gives": self.gives,
            "carrier_one": self.carrier_one,
            "carrier_one_judged": self.carrier_one_judged,
            "carrier_one_enveloped": self.carrier_one_enveloped,
            "carrier_one_elsewhere": self.carrier_one_elsewhere,
            "min_players": self.min_players,
            "division": division,
        });
        if !self.complete() {
            doc["unbound"] = json!(true);
            doc["reason"] = json!(
                "an AND-join or a judgeable `carrier: one` give has no template in the suite"
            );
        }
        doc
    }
}

/// Count the party-division checks off the emitted suite in `out`.
pub fn party_binding(plan: &Plan, out: &BuildOutput) -> PartyBinding {
    let c = plan.campaign;
    let ns = &plan.namespace;
    let test_dir = format!("packtest-datapack/data/{ns}/test/");
    let in_suite: Vec<&str> = out
        .keys()
        .filter_map(|k| k.strip_prefix(&test_dir))
        .filter(|k| k.ends_with(".mcfunction"))
        .collect();

    let mut b = PartyBinding {
        objectives: c
            .quests
            .content
            .quests
            .iter()
            .map(|q| q.objectives.len())
            .sum(),
        templates: in_suite.len(),
        party_join_templates: in_suite
            .iter()
            .filter(|k| k.starts_with("party_join_"))
            .count(),
        min_players: plan::min_players(c) as usize,
        ..PartyBinding::default()
    };

    let joins = objective::and_joins(c);
    b.and_joins = joins.len();
    b.and_joins_bound = joins
        .iter()
        .filter(|(_, o)| {
            out.contains_key(&objective::party_join_template_path(ns, o.id().as_str()))
        })
        .count();

    delvewright_dsl::effects::for_each_campaign_effect(c, &mut |_, _, e| {
        if matches!(e.verb, Verb::GiveItem { .. }) {
            b.gives += 1;
            if e.gives_to_one() {
                b.carrier_one += 1;
            }
        }
    });
    for q in &c.quests.content.quests {
        for o in &q.objectives {
            let oid = o.id().as_str();
            let top: Vec<&QuestEffect> = objective_effects(c, oid)
                .into_iter()
                .filter(|e| e.gives_to_one())
                .collect();
            let template = out
                .get(&carrier_template_path(ns, oid))
                .map(|t| String::from_utf8_lossy(t).into_owned());
            for e in top {
                if !judgeable(e) {
                    b.carrier_one_enveloped += 1;
                    continue;
                }
                let Some(item) = e.give_item() else { continue };
                if template.as_deref().is_some_and(|t| {
                    t.contains(&format!("run clear @s {item} 0")) && t.contains("assert score #pc_")
                }) {
                    b.carrier_one_judged += 1;
                }
            }
        }
    }
    b.carrier_one_elsewhere = b
        .carrier_one
        .saturating_sub(b.carrier_one_enveloped)
        .saturating_sub(
            judged_carrier_gives(c)
                .iter()
                .map(|(_, g)| g.len())
                .sum::<usize>(),
        );

    if b.min_players >= 2 {
        let flow = crate::compiler::flow::Flow::new(c);
        b.division = Some(
            flow.divide(&flow.playthrough(), b.min_players)
                .map_err(|f| f.message()),
        );
    }
    b
}

// ------------------------------------------------ the absent member (spec-0141) --

/// The first `collect` that asks for two or more of its item, with its quest:
/// the one a party can hold between two inventories.
fn first_split_collect(c: &delvewright_dsl::Campaign) -> Option<(&str, &Objective)> {
    c.quests.content.quests.iter().find_map(|q| {
        q.objectives
            .iter()
            .find(|o| matches!(o, Objective::Collect { count, .. } if *count >= 2))
            .map(|o| (q.id.as_str(), o))
    })
}

/// Generated **party-count** PackTest (spec-0141 §3.1): a `collect` of `n`
/// completes when two members hold `n` between them.
///
/// The claim a single dummy cannot make: one player's count and the party's
/// are the same number to a party of one. The template spawns a second member,
/// hands its own dummy `n − 1` of the stack and the other member one, opens the
/// objective's guards and runs one real `tick`.
///
/// Batch model: own member (spawned and removed here), own dummy pinned, the
/// item cleared from every other player first, and no `await`.
pub(super) fn emit_party_collect_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let Some((qid, o)) = first_split_collect(c) else {
        return;
    };
    let Objective::Collect { id, count, .. } = o else {
        return;
    };
    let stack = collect_stack(o).expect("a collect has a stack");
    let item = collect_predicate(o).expect("a collect has a predicate");
    // The stack without its count: the two members are handed parts of it.
    let Some((bare, _)) = stack.rsplit_once(' ') else {
        return;
    };
    let (pin, sel) = pin_dummy("dw_t_pcol");
    let other = "dwpcolp1";
    let other_sel = format!("@a[name={other},limit=1]");
    let party = plan::PARTY;
    let mut b = packtest_header(&format!(
        "{}: collect `{id}` counts the party — {} held by one member and 1 by another complete \
         it (spec-0141)",
        artifact_title(c),
        count - 1
    ));
    b.push(format!("function {ns}:setup"));
    b.push(pin);
    b.push(format!("dummy {other} spawn"));
    b.extend(objective::clear_from_others("dw_t_pcol", &[o]));
    b.push(format!("clear {sel} {item}"));
    b.push(format!(
        "scoreboard players set {party} {} 0",
        obj_score(id.as_str())
    ));
    b.push(format!("give {sel} {bare} {}", count - 1));
    b.push(format!("give {other_sel} {bare} 1"));
    b.extend(packtest_guards(plan, qid, o, true));
    b.push(format!("function {ns}:tick"));
    b.push(format!(
        "assert score {party} {} matches 1",
        obj_score(id.as_str())
    ));
    b.push(format!("clear {sel} {item}"));
    b.push(format!("clear {other_sel} {item}"));
    b.push(format!("dummy {other} leave"));
    out.insert(party_collect_template_path(ns), lines(&b).into_bytes());
}

fn party_collect_template_path(ns: &str) -> String {
    format!("packtest-datapack/data/{ns}/test/party_collect.mcfunction")
}

/// Generated **owed-grant** PackTests (spec-0141 §3.2), one per grant: a
/// member who arrives after the party was handed a stack is handed theirs.
///
/// The template issues the grant through its real `grant_<i>`, spawns a member
/// who was not there, and runs the owed dispatch as that member: they hold the
/// stack once, a second dispatch owes them nothing for it, and their count of
/// it is the party's.
///
/// Batch model: own member under a name no other template uses, own scratch
/// holders (`#pg_<i>_*`), the member's copies cleared before they leave, and
/// no `await`.
pub(super) fn emit_party_grant_packtests(plan: &Plan, out: &mut BuildOutput) {
    use super::super::grant::{
        GRANT_OWED_FN, GRANT_PARTY, grant_fn, grant_objective, grant_owed_fn, grants,
    };
    let ns = &plan.namespace;
    let c = plan.campaign;
    let all = grants(plan);
    for (i, stack) in all.iter().enumerate() {
        let Some((bare, n)) = stack.rsplit_once(' ') else {
            continue;
        };
        // The item id alone: `clear … 0` counts every stack of it the member holds.
        let item = bare.split('[').next().unwrap_or(bare);
        let obj = grant_objective(i);
        let other = format!("dwpg{i}p1");
        let sel = format!("@a[name={other},limit=1]");
        let mut b = packtest_header(&format!(
            "{}: grant {i} (`{item}` × {n}) is owed to a member who arrives after it was issued \
             (spec-0141)",
            artifact_title(c)
        ));
        b.push(format!("function {ns}:setup"));
        b.push(format!("function {ns}:{}", grant_fn(i)));
        b.push(format!("dummy {other} spawn"));
        // Own init: a name's scores outlive its body, so the arriving member's
        // count of this grant is put one issue behind the party's.
        b.push(format!("clear {sel} {item}"));
        // Level with the party on every grant, then one issue of this one
        // behind: the dispatch owes them this stack and no other.
        for j in 0..all.len() {
            let oj = grant_objective(j);
            b.push(format!(
                "scoreboard players operation {sel} {oj} = {GRANT_PARTY} {oj}"
            ));
        }
        b.push(format!("scoreboard players remove {sel} {obj} 1"));
        b.push(format!(
            "execute as {sel} run function {ns}:{GRANT_OWED_FN}"
        ));
        b.push(format!(
            "execute store result score #pg_{i}_a dw.sys run clear {sel} {item} 0"
        ));
        b.push(format!("assert score #pg_{i}_a dw.sys matches {n}"));
        b.push(format!(
            "execute store success score #pg_{i}_l dw.sys if score {sel} {obj} = {GRANT_PARTY} {obj}"
        ));
        b.push(format!("assert score #pg_{i}_l dw.sys matches 1"));
        // Level with the party, the member is owed nothing more of it.
        b.push(format!(
            "execute as {sel} run function {ns}:{GRANT_OWED_FN}"
        ));
        b.push(format!(
            "execute store result score #pg_{i}_b dw.sys run clear {sel} {item} 0"
        ));
        b.push(format!("assert score #pg_{i}_b dw.sys matches {n}"));
        b.push(format!("clear {sel}"));
        b.push(format!("dummy {other} leave"));
        // The hand-over itself, named so the suite calls every body it emits.
        b.push(format!("# hands over through {ns}:{}", grant_owed_fn(i)));
        out.insert(party_grant_template_path(ns, i), lines(&b).into_bytes());
    }
}

fn party_grant_template_path(ns: &str, i: usize) -> String {
    format!("packtest-datapack/data/{ns}/test/party_grant_{i}.mcfunction")
}

/// The first checkpoint a class apply can be told from the entry point by:
/// one whose seat is in another column than the campaign's spawn.
fn first_distinct_checkpoint(plan: &Plan) -> Option<(usize, [i32; 3])> {
    let entry = campaign_spawn(plan)?;
    plan.checkpoints
        .iter()
        .map(|cp| (cp.index, plan.seat(cp)))
        .find(|(_, seat)| seat[0] != entry[0] || seat[2] != entry[2])
}

/// Generated **late-class** PackTest (spec-0141 §3.3): a player who takes a
/// class while a checkpoint is armed arrives on its seat, not at the entry
/// point.
///
/// Batch model: the framework dummy (`@s`), the active-checkpoint marker set
/// and put back, and every residue of the class apply cleared, as
/// `class_apply_<c>` clears it.
pub(super) fn emit_party_class_packtest(plan: &Plan, out: &mut BuildOutput) {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let (Some(cl), Some(class)) = (plan.classes.first(), c.classes.content.classes.first()) else {
        return;
    };
    let Some(entry) = campaign_spawn(plan) else {
        return;
    };
    let Some((index, seat)) = first_distinct_checkpoint(plan) else {
        return;
    };
    let safe = &cl.safe;
    let mut b = packtest_header(&format!(
        "{}: a class taken while checkpoint {index} is armed arrives on its seat (spec-0141)",
        artifact_title(c)
    ));
    b.push(format!("function {ns}:setup"));
    b.push("scoreboard players reset @s dw.class".to_string());
    b.push("scoreboard players reset @s dw.classed".to_string());
    b.push(format!("scoreboard players set #cp dw.sys {index}"));
    b.push(format!("tp @s {} {} {}", entry[0] + 32, entry[1], entry[2]));
    b.push(format!(
        "execute as @s run function {ns}:class_apply_{safe}"
    ));
    b.push("execute store result score #pcl_x dw.sys run data get entity @s Pos[0] 1".to_string());
    b.push(format!("assert score #pcl_x dw.sys matches {}", seat[0]));
    b.push("execute store result score #pcl_z dw.sys run data get entity @s Pos[2] 1".to_string());
    b.push(format!("assert score #pcl_z dw.sys matches {}", seat[2]));
    b.push("scoreboard players set #cp dw.sys -1".to_string());
    b.push("clear @s".to_string());
    b.push(format!("tag @s remove {}", class_tag(safe)));
    b.push("scoreboard players reset @s dw.classed".to_string());
    for (k, item) in class.kit.iter().enumerate() {
        if matches!(item.carrier, Some(delvewright_dsl::Carrier::One)) {
            b.push(format!("scoreboard players reset #kit_{safe}_{k} dw.sys"));
        }
    }
    out.insert(party_class_template_path(ns), lines(&b).into_bytes());
}

fn party_class_template_path(ns: &str) -> String {
    format!("packtest-datapack/data/{ns}/test/party_class_arrives.mcfunction")
}

/// The distance within which a party template counts a body as standing on a
/// seat. Two seats can be one cell apart, so a body midway between their
/// centres stands half a cell from each: a reach of `..0.5` counts it on both,
/// and one member put between two seats reads as two. A carry puts a body on
/// the centre itself. Every count a party template makes is of its own
/// members (a tag it puts on them): a test's dummies spawn by the world's
/// spawn, which can be the mark, and another batch's dummy standing there is
/// not this template's member.
const ON_SEAT: &str = "..0.25";

/// The lines that count a party template's own two members (`ours`) seated
/// on a carry's seats, one count per seat, into `holder`, and assert two.
///
/// Every seat is read, not the first two: the members present are numbered in
/// `@a`'s order, and a body another test left on the server is numbered too,
/// so which seats this template's members take is not its to say. That they
/// are both seated, each on a centre of its own, is.
fn seated_members(holder: &str, ours: &str, seats: &[[i32; 3]]) -> Vec<String> {
    let mut out = vec![format!("scoreboard players set {holder} dw.sys 0")];
    for seat in seats {
        let [sx, sy, sz] = super::super::coords::ent_xyz(*seat);
        out.push(format!(
            "execute positioned {sx} {sy} {sz} as @a[tag={ours},distance={ON_SEAT}] run \
             scoreboard players add {holder} dw.sys 1"
        ));
    }
    out.push(format!("assert score {holder} dw.sys matches 2"));
    out
}

/// Generated **party-teleport** PackTests (DSL v0.88), one per distinct
/// `teleport` whose subject is the party: two members standing far apart, on
/// either side of the mark and well out of reach of it, are both carried, each
/// onto a seat of their own.
///
/// The claim a single dummy cannot make: `tp @s` and a move of every member
/// are the same command to a party of one. The template spawns a second
/// member, puts the two 48 blocks either side of the mark, asserts nobody
/// stands within 8 blocks of it, runs the campaign's real `teleport_<key>`,
/// and counts one player on each of the first two seats.
///
/// Batch model: the teleport moves every player on the server, so the
/// template runs alone, in its own test environment
/// ([`super::limit::alone`]); own member, own scratch holders
/// (`#ptp_<k>_*`), no `await`.
pub(super) fn emit_party_teleport_packtests(plan: &Plan, out: &mut BuildOutput) {
    use super::super::teleport::{party_seats, teleport_cell, teleport_fn};
    let ns = &plan.namespace;
    let c = plan.campaign;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for eff in all_campaign_effects(c) {
        if eff.party_teleport().is_none() {
            continue;
        }
        let name = teleport_fn(eff);
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(to) = teleport_cell(plan, eff) else {
            continue;
        };
        let seats = party_seats(plan, to);
        let k = seen.len() - 1;
        let (pin, me) = pin_dummy(&format!("dw_t_ptp{k}"));
        let other = format!("dwptp{k}p1");
        let other_sel = format!("@a[name={other},limit=1]");
        let template = format!("party_{name}");
        let mut b = packtest_header(&format!(
            "{}: `{name}` carries every member of the party, wherever they stand, each onto a \
             seat of their own",
            artifact_title(c)
        ));
        b.insert(1, super::limit::alone(ns, &template, out));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!("dummy {other} spawn"));
        let ours = format!("dw_t_ptp{k}m");
        b.push(format!("tag {me} add {ours}"));
        b.push(format!("tag {other_sel} add {ours}"));
        if !plan.classes.is_empty() {
            b.push(format!("scoreboard players set {me} dw.classed 1"));
            b.push(format!("scoreboard players set {other_sel} dw.classed 1"));
        }
        let [x, y, z] = to.map(f64::from);
        b.push(format!("tp {me} {} {y} {}", x + 48.5, z + 0.5));
        b.push(format!("tp {other_sel} {} {y} {}", x - 47.5, z + 0.5));
        // Bound, not assumed: nobody stands near the mark before the move.
        b.push(format!(
            "execute positioned {} {y} {} store result score #ptp_{k}_near dw.sys if entity \
             @a[tag={ours},distance=..8]",
            x + 0.5,
            z + 0.5
        ));
        b.push(format!("assert score #ptp_{k}_near dw.sys matches 0"));
        b.push(format!("function {ns}:{name}"));
        b.extend(seated_members(&format!("#ptp_{k}_on"), &ours, seats));
        b.push(format!("dummy {other} leave"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{template}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// Generated **party-crossing** PackTests, one per area the exported path
/// crosses into: the crossing an objective's completion performs carries
/// every member, wherever they stand, and a member who was away is carried
/// once they are back.
///
/// The claim a single dummy cannot make: `teleport @s` and a carry of every
/// member are the same command to a party of one. The template spawns a
/// second member, puts the two 48 blocks either side of the area's entry
/// point, asserts nobody stands within 8 blocks of it, has the pinned dummy
/// complete the first objective that crosses there through its real
/// `complete_<obj>`, and counts one player on each of the first two seats.
/// Then a third member, spawned after the crossing and standing far off, runs
/// the owed function the tick runs for a member behind the party's crossings,
/// and is found on the entry point.
///
/// Batch model: the crossing moves every player on the server, so the
/// template runs alone ([`super::limit::alone`]); own members, own scratch
/// holders (`#pcx_<k>_*`), no `await`.
pub(super) fn emit_party_crossing_packtests(plan: &Plan, out: &mut BuildOutput) {
    use super::super::teleport::{CROSS_OWED_FN, crossings, party_seats};
    let ns = &plan.namespace;
    let c = plan.campaign;
    for (k, x) in crossings(plan, &BTreeMap::new()).iter().enumerate() {
        let Some(oid) = plan
            .transport
            .iter()
            .find(|(_, pos)| **pos == x.to)
            .map(|(oid, _)| oid.as_str())
        else {
            continue;
        };
        let seats = party_seats(plan, x.to);
        let (pin, me) = pin_dummy(&format!("dw_t_pcx{k}"));
        let other = format!("dwpcx{k}p1");
        let late = format!("dwpcx{k}p2");
        let other_sel = format!("@a[name={other},limit=1]");
        let late_sel = format!("@a[name={late},limit=1]");
        let template = format!("party_{}", x.fn_name());
        let mut b = packtest_header(&format!(
            "{}: completing `{oid}` carries every member of the party into `{}`, each onto a \
             seat of their own, and a member who was away is carried once they are back",
            artifact_title(c),
            x.area
        ));
        b.insert(1, super::limit::alone(ns, &template, out));
        b.push(format!("function {ns}:setup"));
        b.push(pin);
        b.push(format!("dummy {other} spawn"));
        let ours = format!("dw_t_pcx{k}m");
        b.push(format!("tag {me} add {ours}"));
        b.push(format!("tag {other_sel} add {ours}"));
        if !plan.classes.is_empty() {
            b.push(format!("scoreboard players set {me} dw.classed 1"));
            b.push(format!("scoreboard players set {other_sel} dw.classed 1"));
        }
        let [tx, ty, tz] = x.to.map(f64::from);
        b.push(format!("tp {me} {} {ty} {}", tx + 48.5, tz + 0.5));
        b.push(format!("tp {other_sel} {} {ty} {}", tx - 47.5, tz + 0.5));
        // Bound, not assumed: nobody stands near the entry point before.
        b.push(format!(
            "execute positioned {} {ty} {} store result score #pcx_{k}_near dw.sys if entity \
             @a[tag={ours},distance=..8]",
            tx + 0.5,
            tz + 0.5
        ));
        b.push(format!("assert score #pcx_{k}_near dw.sys matches 0"));
        b.push(format!(
            "execute as {me} run function {ns}:complete_{}",
            super::super::objective::safe_obj_fn(oid)
        ));
        b.extend(seated_members(&format!("#pcx_{k}_on"), &ours, seats));
        // A member who was away: spawned after the crossing, far off.
        b.push(format!("dummy {late} spawn"));
        if !plan.classes.is_empty() {
            b.push(format!("scoreboard players set {late_sel} dw.classed 1"));
        }
        b.push(format!("tp {late_sel} {} {ty} {}", tx - 47.5, tz + 0.5));
        b.push(format!(
            "execute as {late_sel} run function {ns}:{CROSS_OWED_FN}"
        ));
        let [sx, sy, sz] = super::super::coords::ent_xyz(seats[0]);
        b.push(format!(
            "execute positioned {sx} {sy} {sz} store result score #pcx_{k}_late dw.sys if entity \
             @a[name={late},limit=1,distance={ON_SEAT}]"
        ));
        b.push(format!("assert score #pcx_{k}_late dw.sys matches 1"));
        b.push(format!("dummy {late} leave"));
        b.push(format!("dummy {other} leave"));
        out.insert(
            format!("packtest-datapack/data/{ns}/test/{template}.mcfunction"),
            lines(&b).into_bytes(),
        );
    }
}

/// What the absent-member templates bound on one build (spec-0141 §5), counted
/// off the emitted suite.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AbsentBinding {
    /// `collect` objectives that ask for two or more of their item.
    pub split_collects: usize,
    /// Whether the suite carries `party_collect`.
    pub split_collect_judged: bool,
    /// Grants the campaign issues.
    pub grants: usize,
    /// Of those, the ones with a `party_grant_<i>` template.
    pub grants_judged: usize,
    /// Checkpoints whose seat is in another column than the entry point.
    pub distinct_checkpoints: usize,
    /// Whether the suite carries `party_class_arrives`.
    pub class_judged: bool,
    /// Limited objectives whose clock redraws its bar at least once.
    pub limits: usize,
    /// Of those, the ones with a `party_limit_bar_<obj>` template.
    pub limits_judged: usize,
}

impl AbsentBinding {
    /// Whether every surface the campaign declares has its template.
    pub fn complete(&self, has_classes: bool) -> bool {
        (self.split_collects == 0 || self.split_collect_judged)
            && self.grants_judged == self.grants
            && (self.distinct_checkpoints == 0 || !has_classes || self.class_judged)
            && self.limits_judged == self.limits
    }

    /// The one line a build prints.
    pub fn line(&self) -> String {
        format!(
            "absent-member binding (spec-0141): {} `collect`(s) of two or more, {} judged by the \
             two-member `party_collect` PackTest; {} grant(s), {} judged by a late-member \
             `party_grant` PackTest; {} checkpoint(s) off the entry column, {} judged by \
             `party_class_arrives`; {} limit clock(s) that redraw, {} judged by a late-member \
             `party_limit_bar` PackTest",
            self.split_collects,
            usize::from(self.split_collect_judged),
            self.grants,
            self.grants_judged,
            self.distinct_checkpoints,
            usize::from(self.class_judged),
            self.limits,
            self.limits_judged,
        )
    }
}

/// Count the absent-member templates off the emitted suite in `out`.
pub fn absent_binding(plan: &Plan, out: &BuildOutput) -> AbsentBinding {
    let ns = &plan.namespace;
    let c = plan.campaign;
    let all = super::super::grant::grants(plan);
    let limits: Vec<&str> = c
        .quests
        .content
        .quests
        .iter()
        .flat_map(|q| &q.objectives)
        .filter(|o| o.limit().is_some_and(|l| super::limit::redraws(l.ticks)))
        .map(|o| o.id().as_str())
        .collect();
    let distinct = campaign_spawn(plan).map_or(0, |entry| {
        plan.checkpoints
            .iter()
            .map(|cp| plan.seat(cp))
            .filter(|s| s[0] != entry[0] || s[2] != entry[2])
            .count()
    });
    AbsentBinding {
        split_collects: c
            .quests
            .content
            .quests
            .iter()
            .flat_map(|q| &q.objectives)
            .filter(|o| matches!(o, Objective::Collect { count, .. } if *count >= 2))
            .count(),
        split_collect_judged: out.contains_key(&party_collect_template_path(ns)),
        grants: all.len(),
        grants_judged: (0..all.len())
            .filter(|i| out.contains_key(&party_grant_template_path(ns, *i)))
            .count(),
        distinct_checkpoints: distinct,
        class_judged: out.contains_key(&party_class_template_path(ns)),
        limits: limits.len(),
        limits_judged: limits
            .iter()
            .filter(|oid| {
                out.contains_key(&format!(
                    "packtest-datapack/data/{ns}/test/{}.mcfunction",
                    super::limit::party_limit_bar_name(oid)
                ))
            })
            .count(),
    }
}
