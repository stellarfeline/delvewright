//! `DW0847`: a gate whose own terms contradict each other can never open, so
//! whatever it guards is authored content that provably never happens.
//!
//! One rule over the whole closed consumer set (`gate::for_each_gate`), because
//! satisfiability is a property of the GATE — the shared object class — and a
//! check written beside the first verb that needed it (the cast ladder's
//! per-clause solver) would leave the other six classes with no surface. What
//! this file pins down:
//!
//! * a flag on both `requires_flags` and `forbids_flags` is refused, at any
//!   version (both fields exist from v0.6);
//! * `requires_state` terms on one datum with an empty intersection are
//!   refused, on an objective and on a cast placement (the consumer whose
//!   ladder solver motivated the rule);
//! * a tight-but-satisfiable multi-term gate is NOT refused — the check is
//!   satisfiability, never strictness.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

fn campaign_with(quests: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

fn codes(raw: &RawCampaign) -> Vec<String> {
    check_campaign(raw)
        .into_iter()
        .map(|d| d.code)
        .collect::<Vec<_>>()
}

/// A v0.10 quests document whose one datum is written and read, with the
/// objective's `requires_state` terms substituted per test.
fn quests_doc(objective_terms: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "state": [
      {{ "id": "state/toll", "scope": "party", "note": "coins still owed" }}
    ],
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"],
            "requires_state": {objective_terms} }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "add-state", "state": "state/toll", "amount": -1 }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

/// `at-least 5` with `at-most 3` on one datum: no integer satisfies both.
#[test]
fn contradictory_state_terms_on_an_objective_are_refused() {
    let raw = campaign_with(&quests_doc(
        r#"[ {"state": "state/toll", "op": "at-least", "value": 5},
             {"state": "state/toll", "op": "at-most", "value": 3} ]"#,
    ));
    assert!(
        codes(&raw).contains(&"DW0847".to_string()),
        "an empty intersection must be refused: {:?}",
        codes(&raw)
    );
}

/// Two different `equals` pins on one datum contradict; `not-equals` punching
/// out the only pinned value does too.
#[test]
fn a_pin_conflict_is_refused() {
    for terms in [
        r#"[ {"state": "state/toll", "op": "equals", "value": 2},
             {"state": "state/toll", "op": "equals", "value": 3} ]"#,
        r#"[ {"state": "state/toll", "op": "equals", "value": 2},
             {"state": "state/toll", "op": "not-equals", "value": 2} ]"#,
    ] {
        let raw = campaign_with(&quests_doc(terms));
        assert!(
            codes(&raw).contains(&"DW0847".to_string()),
            "no value satisfies {terms}: {:?}",
            codes(&raw)
        );
    }
}

/// A tight gate that still admits a value is NOT a contradiction: `[3, 5]`
/// minus `{{4}}` keeps 3 and 5. The check is satisfiability, never strictness.
#[test]
fn a_satisfiable_multi_term_gate_is_clean() {
    let raw = campaign_with(&quests_doc(
        r#"[ {"state": "state/toll", "op": "at-least", "value": 3},
             {"state": "state/toll", "op": "at-most", "value": 5},
             {"state": "state/toll", "op": "not-equals", "value": 4} ]"#,
    ));
    assert!(
        !codes(&raw).contains(&"DW0847".to_string()),
        "a satisfiable gate must not be refused: {:?}",
        codes(&raw)
    );
}

/// The flag axis, on a v0.7 trigger: requiring and forbidding one flag is the
/// same emptiness one axis over, and needs no v0.10 surface to commit.
#[test]
fn a_flag_required_and_forbidden_at_once_is_refused() {
    let quests = r#"{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [ { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" } ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "set-flag", "flag": "flag/paid" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "triggers": [
      {
        "id": "trigger/toll-box",
        "at": "anchor/exit",
        "on": { "on": "strike" },
        "requires_flags": ["flag/paid"],
        "forbids_flags": ["flag/paid"],
        "effects": [ { "type": "narrate", "text": "The box rattles." } ]
      }
    ]
  }
}"#;
    let raw = campaign_with(quests);
    assert!(
        codes(&raw).contains(&"DW0847".to_string()),
        "requires+forbids of one flag must be refused: {:?}",
        codes(&raw)
    );
}

/// The consumer that motivated the rule: a cast placement whose gate can never
/// hold declares a branch scene no branch can ever reach.
#[test]
fn a_contradictory_cast_placement_gate_is_refused() {
    let quests = r#"{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "state": [ { "id": "state/toll", "scope": "party", "note": "coins still owed" } ],
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [ { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" } ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "add-state", "state": "state/toll", "amount": -1 } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ],
        "cast": {
          "npc/keeper": [
            { "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" },
            { "at": "anchor/keeper-stand", "doing": "counting a toll that cannot exist",
              "dialogue": { "barks": ["Impossible."] },
              "requires_state": [
                { "state": "state/toll", "op": "at-least", "value": 5 },
                { "state": "state/toll", "op": "at-most", "value": 3 }
              ] }
          ]
        }
      }
    ]
  }
}"#;
    let raw = campaign_with(quests);
    assert!(
        codes(&raw).contains(&"DW0847".to_string()),
        "a cast placement's contradictory gate must be refused: {:?}",
        codes(&raw)
    );
}
