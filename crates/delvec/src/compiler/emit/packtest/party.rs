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
                    Verb::GiveItem { item, count, .. } => Some((item.as_str(), *count)),
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
