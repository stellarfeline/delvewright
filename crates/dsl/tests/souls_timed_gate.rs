//! spec-0016 §4 — the stage-5 `timed_gates` section.
//!
//! The DSL layer owns the structural half (`DW0377`): ids, a cycle that actually
//! cycles, a phase inside the cycle, and one owner per gate region. The design
//! half — that the gate is a timing *read* and not a coin flip — needs the nav
//! model's crossing time and lives in `compiler::nav` (`DW0378`).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

const QUESTS_V06: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": { "obj/talk": [] },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "timed_gates": [
      { "id": "timed-gate/inner-door", "gate": "anchor/door", "open_ticks": 60, "closed_ticks": 40 }
    ]
  }
}"#;

fn campaign_with_quests(quests: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
}

/// A well-formed timed gate validates clean under 0.6.0.
#[test]
fn timed_gate_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 timed gate, got: {diags:#?}"
    );
}

/// The section under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn timed_gates_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0141" && d.path == "/content/timed_gates"),
        "the timed_gates section must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// A half-cycle of zero is not a timing gate at all — that is `open-gate` /
/// `close-gate`. `DW0377`.
#[test]
fn zero_length_half_cycle_is_dw0377() {
    for field in ["open_ticks", "closed_ticks"] {
        let bad = QUESTS_V06
            .replace(&format!("\"{field}\": 60"), &format!("\"{field}\": 0"))
            .replace(&format!("\"{field}\": 40"), &format!("\"{field}\": 0"));
        assert_ne!(bad, QUESTS_V06, "{field} substitution must apply");
        let diags = check_campaign(&campaign_with_quests(&bad));
        assert!(
            diags.iter().any(|d| d.code == "DW0377"),
            "{field}: 0 must be DW0377: {diags:#?}"
        );
    }
}

/// A phase is an offset INTO the cycle, so one at or beyond it is `DW0377`.
#[test]
fn phase_beyond_the_cycle_is_dw0377() {
    let bad = QUESTS_V06.replace(
        "\"closed_ticks\": 40 }",
        "\"closed_ticks\": 40, \"phase\": 100 }",
    );
    assert_ne!(bad, QUESTS_V06);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0377"),
        "a phase at the full cycle must be DW0377: {diags:#?}"
    );
}

/// Two clocks over one region race every tick — the region's state would be
/// emission order, not design. `DW0377`.
#[test]
fn two_clocks_on_one_gate_is_dw0377() {
    let bad = QUESTS_V06.replace(
        "{ \"id\": \"timed-gate/inner-door\", \"gate\": \"anchor/door\", \"open_ticks\": 60, \"closed_ticks\": 40 }",
        "{ \"id\": \"timed-gate/inner-door\", \"gate\": \"anchor/door\", \"open_ticks\": 60, \"closed_ticks\": 40 },\n      { \"id\": \"timed-gate/inner-door-b\", \"gate\": \"anchor/door\", \"open_ticks\": 20, \"closed_ticks\": 20 }",
    );
    assert_ne!(bad, QUESTS_V06);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0377"),
        "two clocks on one gate must be DW0377: {diags:#?}"
    );
}

/// A shortcut opens permanently; a clock would re-seal it every cycle — exactly
/// what `DW0372` forbids. Declaring both on one gate is `DW0377`.
#[test]
fn a_shortcut_gate_on_a_clock_is_dw0377() {
    let bad = QUESTS_V06.replace(
        "    \"timed_gates\": [",
        "    \"shortcuts\": [ { \"id\": \"shortcut/inner-door\", \"gate\": \"anchor/door\", \"unlock\": \"anchor/exit\" } ],\n    \"timed_gates\": [",
    );
    assert_ne!(bad, QUESTS_V06);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0377"),
        "a gate that is both a shortcut and a clock must be DW0377: {diags:#?}"
    );
}

/// A malformed timed-gate id is `DW0377`.
#[test]
fn malformed_timed_gate_id_is_dw0377() {
    let bad = QUESTS_V06.replace("\"timed-gate/inner-door\"", "\"Inner Door\"");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0377"),
        "a malformed timed-gate id must be DW0377: {diags:#?}"
    );
}
