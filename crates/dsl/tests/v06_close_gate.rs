//! DSL v0.6 `close-gate` (the physical dual of `open-gate`): validates under
//! `dsl_version 0.6.0` and is reserved (`DW0141`) earlier; an unresolved gate
//! anchor is `DW0142` (the fill-block declaration is a compiler-side check,
//! `DW0343`). Built on the hello-world casting/dialogue (unchanged).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 quests document that opens `anchor/door` then re-seals it with a
/// `close-gate` after the exit is reached (on_complete — nothing left to walk).
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
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [
          { "type": "close-gate", "anchor": "anchor/door" },
          { "type": "campaign-complete" }
        ]
      }
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

/// `close-gate` validates clean under `dsl_version 0.6.0`.
#[test]
fn close_gate_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 close-gate, got: {diags:#?}"
    );
}

/// `close-gate` under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn close_gate_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "close-gate must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// A `close-gate` on an anchor the bound prefab does not provide is `DW0142`
/// (same anchor-existence check as `open-gate`).
#[test]
fn close_gate_unresolved_anchor_is_dw0142() {
    let bad = QUESTS_V06.replace(
        r#"{ "type": "close-gate", "anchor": "anchor/door" }"#,
        r#"{ "type": "close-gate", "anchor": "anchor/nope" }"#,
    );
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0142"),
        "an unresolved close-gate anchor must be DW0142: {diags:#?}"
    );
}
