//! DSL v0.6 `close-gate` (the physical dual of `open-gate`): validates under
//! `dsl_version 0.6.0` and is reserved (`DW0141`) earlier; an unresolved gate
//! anchor is `DW0142` (the fill-block declaration is a compiler-side check,
//! `DW0343`). Built on the hello-world casting/dialogue (unchanged).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 quests document that opens `anchor/door` then re-seals it with a
/// `close-gate` after the exit is reached (on_complete — nothing left to walk).
const QUESTS_V06: &str = r#"{
  "dsl_version": "0.19.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "happening": { "verb": "learns", "text": "the party asks the keeper" },
        "cast": {
          "npc/keeper": { "at": "anchor/keeper-stand", "doing": "keeping the door", "dialogue": "dlg/greeting" }
        },
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper",
            "happening": { "verb": "learns", "text": "the keeper is asked" } },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"],
            "happening": { "verb": "arrives", "text": "the party reaches the exit" } }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door",
                          "happening": { "verb": "opens", "text": "the door opens" } } ]
        },
        "on_complete": [
          { "type": "close-gate", "anchor": "anchor/door", "sealed_hint": "The bars are down for good.",
            "happening": { "verb": "seals", "text": "the door seals behind the party" } },
          { "type": "campaign-complete",
            "happening": { "verb": "survives", "text": "the delve is complete" } }
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
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
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

/// A `close-gate` on an anchor the bound prefab does not provide is `DW0142`
/// (same anchor-existence check as `open-gate`).
#[test]
fn close_gate_unresolved_anchor_is_dw0142() {
    let bad = QUESTS_V06.replace(
        r#"{ "type": "close-gate", "anchor": "anchor/door","#,
        r#"{ "type": "close-gate", "anchor": "anchor/nope","#,
    );
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0142"),
        "an unresolved close-gate anchor must be DW0142: {diags:#?}"
    );
}
