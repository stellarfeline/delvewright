//! spec-0016 §2 — the stage-5 `shortcuts` section.
//!
//! A shortcut is the souls loop-back: a gate sealed from world-load that a
//! far-side mechanism opens **permanently**. The DSL layer owns two of the
//! pattern's three obligations — the declaration must resolve (`DW0371`), and
//! nothing may re-seal it (`DW0372`). The third (the long route actually exists
//! and opening the gate pays) is geometric and lives in the compiler's nav proofs
//! (`DW0373`/`DW0374`).

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
    "shortcuts": [
      { "id": "shortcut/inner-door", "gate": "anchor/door", "unlock": "anchor/exit" }
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

/// A well-formed shortcut validates clean under 0.6.0.
#[test]
fn shortcut_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 shortcut, got: {diags:#?}"
    );
}

/// The `shortcuts` section under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn shortcuts_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0141" && d.path == "/content/shortcuts"),
        "the shortcuts section must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// A malformed shortcut id is `DW0371`.
#[test]
fn malformed_shortcut_id_is_dw0371() {
    let bad = QUESTS_V06.replace("\"shortcut/inner-door\"", "\"Inner Door\"");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0371"),
        "a malformed shortcut id must be DW0371: {diags:#?}"
    );
}

/// A `gate`/`unlock` anchor no prefab exposes is `DW0371` — a shortcut the
/// compiler cannot place is never silently dropped.
#[test]
fn invented_shortcut_anchor_is_dw0371() {
    let bad = QUESTS_V06.replace(
        "\"unlock\": \"anchor/exit\"",
        "\"unlock\": \"anchor/invented\"",
    );
    assert_ne!(bad, QUESTS_V06);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0371" && d.path.ends_with("/unlock")),
        "an invented unlock anchor must be DW0371: {diags:#?}"
    );
}

/// The mechanism must sit on the far side, not in the doorway it opens: an
/// `unlock` equal to the `gate` is `DW0371`.
#[test]
fn unlock_on_its_own_gate_is_dw0371() {
    let bad = QUESTS_V06.replace("\"unlock\": \"anchor/exit\"", "\"unlock\": \"anchor/door\"");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0371"),
        "an unlock on its own gate anchor must be DW0371: {diags:#?}"
    );
}

/// Permanence is structural: a `close-gate` anywhere — including one buried in a
/// nested timeline — that targets a shortcut gate is `DW0372`.
#[test]
fn close_gate_on_a_shortcut_gate_is_dw0372() {
    let bad = QUESTS_V06.replace(
        "\"on_objective_complete\": { \"obj/talk\": [] }",
        "\"on_objective_complete\": { \"obj/talk\": [ { \"type\": \"sequence\", \"steps\": [ { \"at_ticks\": 10, \"effects\": [ { \"type\": \"close-gate\", \"anchor\": \"anchor/door\" } ] } ] } ] }",
    );
    assert_ne!(bad, QUESTS_V06);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0372"),
        "re-sealing a shortcut gate must be DW0372: {diags:#?}"
    );
}

/// `close-gate` on a NON-shortcut gate stays legal — the point-of-no-return beat
/// is untouched by the shortcut rule.
#[test]
fn close_gate_on_an_unowned_gate_is_still_legal() {
    let no_shortcut = QUESTS_V06
        .replace(
            "\"on_objective_complete\": { \"obj/talk\": [] }",
            "\"on_objective_complete\": { \"obj/talk\": [ { \"type\": \"close-gate\", \"anchor\": \"anchor/door\" } ] }",
        )
        .replace(
            "    \"shortcuts\": [\n      { \"id\": \"shortcut/inner-door\", \"gate\": \"anchor/door\", \"unlock\": \"anchor/exit\" }\n    ]\n",
            "    \"shortcuts\": []\n",
        );
    let diags = check_campaign(&campaign_with_quests(&no_shortcut));
    assert!(
        !diags.iter().any(|d| d.code == "DW0372"),
        "close-gate without a shortcut on that anchor must not trip DW0372: {diags:#?}"
    );
}
