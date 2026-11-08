//! `DW1077` — a quest's stage-5 `trigger` agrees with its stage-4 `depends_on`.
//!
//! Built on the hello-world campaign through [`common::patch_doc`]: the
//! hello-world quest is the finale, and two mandatory quests, a prologue and a
//! side quest, form a join in front of it (the finale `depends_on` both and is
//! armed by one). Each case perturbs one trigger or one `depends_on` list from
//! the green join and states the exact code set it must raise.

use crate::common;

use std::collections::BTreeSet;

use delvewright_dsl::{RawCampaign, check_campaign};
use serde_json::{Value, json};

const FINALE: &str = "quest/open-the-door";
const PROLOGUE: &str = "quest/prologue";
const SIDE: &str = "quest/side";

fn start() -> Value {
    json!({ "type": "campaign-start" })
}

fn after(quest: &str) -> Value {
    json!({ "type": "quest-complete", "quest": quest })
}

/// The campaign: each planned quest's `depends_on` and each expanded quest's
/// trigger, in the order finale, prologue, side.
struct Join {
    deps: [Vec<&'static str>; 3],
    triggers: [Value; 3],
}

impl Default for Join {
    /// The green join: the finale depends on both and is armed by the prologue;
    /// the side quest is the join's other arm.
    fn default() -> Self {
        Join {
            deps: [vec![PROLOGUE, SIDE], vec![], vec![]],
            triggers: [after(PROLOGUE), start(), start()],
        }
    }
}

impl Join {
    fn raw(&self) -> RawCampaign {
        let mut c = common::valid_raw();
        c.quest_plan = common::patch_doc(&c.quest_plan, |v| {
            let quests = v["content"]["quests"].as_array_mut().unwrap();
            assert_eq!(quests.len(), 1, "hello-world plans exactly one quest");
            quests[0]["depends_on"] = json!(self.deps[0]);
            for (id, deps) in [(PROLOGUE, &self.deps[1]), (SIDE, &self.deps[2])] {
                quests.push(json!({
                    "act": 1,
                    "area": "area/keep",
                    "depends_on": deps,
                    "goal": "A beat the finale waits for.",
                    "activity": ["talk"],
                    "id": id,
                    "mandatory": true,
                    "npcs": [],
                }));
            }
        });
        c.quests = common::patch_doc(&c.quests, |v| {
            let quests = v["content"]["quests"].as_array_mut().unwrap();
            assert_eq!(quests.len(), 1, "hello-world expands exactly one quest");
            quests[0]["trigger"] = self.triggers[0].clone();
            for (id, trigger, obj) in [
                (PROLOGUE, &self.triggers[1], "obj/prologue-look"),
                (SIDE, &self.triggers[2], "obj/side-look"),
            ] {
                quests.push(json!({
                    "id": id,
                    "objectives": [{
                        "anchor": "anchor/exit",
                        "id": obj,
                        "radius": 2,
                        "type": "reach-anchor",
                    }],
                    "on_complete": [],
                    "trigger": trigger,
                }));
            }
        });
        c
    }

    fn codes(&self) -> BTreeSet<String> {
        check_campaign(&self.raw())
            .into_iter()
            .map(|d| d.code)
            .collect()
    }

    fn dw1077_paths(&self) -> Vec<String> {
        check_campaign(&self.raw())
            .into_iter()
            .filter(|d| d.code == "DW1077")
            .map(|d| d.path)
            .collect()
    }
}

fn set(codes: &[&str]) -> BTreeSet<String> {
    codes.iter().map(|s| (*s).to_string()).collect()
}

/// The join greens: the finale is armed by one quest its `depends_on` lists,
/// and the other arm is not demanded of the trigger.
#[test]
fn a_join_armed_by_one_of_its_arms_is_green() {
    assert_eq!(Join::default().codes(), BTreeSet::new());
}

/// A quest the plan says depends on others, armed at the start.
#[test]
fn a_quest_with_depends_on_armed_at_the_start_is_dw1077() {
    let j = Join {
        triggers: [start(), start(), start()],
        ..Join::default()
    };
    assert_eq!(j.codes(), set(&["DW1077"]));
    assert_eq!(
        j.dw1077_paths(),
        vec!["/content/quests/0/trigger".to_string()]
    );
}

/// A trigger naming a quest the plan's `depends_on` does not list, and its
/// green half: adding the quest to `depends_on` clears the same document.
#[test]
fn a_trigger_naming_a_quest_absent_from_depends_on_is_dw1077() {
    let j = Join {
        triggers: [after(PROLOGUE), start(), after(PROLOGUE)],
        ..Join::default()
    };
    assert_eq!(j.codes(), set(&["DW1077"]));
    assert_eq!(
        j.dw1077_paths(),
        vec!["/content/quests/2/trigger/quest".to_string()]
    );

    let agreed = Join {
        deps: [vec![PROLOGUE, SIDE], vec![], vec![PROLOGUE]],
        triggers: [after(PROLOGUE), start(), after(PROLOGUE)],
    };
    assert_eq!(agreed.codes(), BTreeSet::new());
}

/// A trigger naming a quest outside a non-empty `depends_on` is refused with
/// the entries the plan does list named, so the author sees which quest the
/// plan says arms it.
#[test]
fn a_refusal_names_the_entries_depends_on_lists() {
    let j = Join {
        deps: [vec![PROLOGUE], vec![], vec![]],
        triggers: [after(SIDE), start(), start()],
    };
    let refusals: Vec<_> = check_campaign(&j.raw())
        .into_iter()
        .filter(|d| d.code == "DW1077")
        .collect();
    assert_eq!(refusals.len(), 1, "{refusals:?}");
    assert_eq!(refusals[0].path, "/content/quests/0/trigger/quest");
    assert!(
        refusals[0].message.contains("lists `quest/prologue`"),
        "{}",
        refusals[0].message
    );
}

/// Every refusal binds to the edge that disagrees and to nothing else: the
/// count of `DW1077` equals the count of disagreeing quests, swept over every
/// trigger the three quests could carry with the plan held at the join.
#[test]
fn dw1077_counts_exactly_the_disagreeing_quests() {
    let choices = || [start(), after(FINALE), after(PROLOGUE), after(SIDE)];
    let ids = [FINALE, PROLOGUE, SIDE];
    let mut examined = 0usize;
    let mut refused = 0usize;
    for t0 in choices() {
        for t1 in choices() {
            for t2 in choices() {
                let j = Join {
                    triggers: [t0.clone(), t1.clone(), t2.clone()],
                    ..Join::default()
                };
                // Self-arming and cyclic triggers are other codes' findings;
                // skip any trigger naming its own quest so the count is about
                // agreement alone.
                if j.triggers
                    .iter()
                    .zip(ids)
                    .any(|(t, id)| t["quest"].as_str() == Some(id))
                {
                    continue;
                }
                let expected = j
                    .triggers
                    .iter()
                    .zip(&j.deps)
                    .filter(|(t, deps)| match t["quest"].as_str() {
                        Some(q) => !deps.contains(&q),
                        None => !deps.is_empty(),
                    })
                    .count();
                assert_eq!(
                    j.dw1077_paths().len(),
                    expected,
                    "triggers {:?}",
                    j.triggers
                );
                examined += 1;
                refused += expected;
            }
        }
    }
    assert_eq!(
        examined, 27,
        "population swept: 3 x 3 x 3 non-self triggers"
    );
    assert!(
        refused > 0,
        "the sweep refused nothing: {refused} of {examined}"
    );
}
