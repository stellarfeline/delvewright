//! DSL v0.6 (spec-0012 checkpoints + spec-0014 stealth): the `set-checkpoint`
//! effect (with `on_respawn`) and the `begin-stealth`/`end-stealth` verbs validate
//! under `0.6.0` and are reserved (`DW0141`) earlier.
//!
//! Built on the hello-world casting/world/dialogue (unchanged, 0.2.0) with a v0.6
//! stage-5 `quests` document — additive verbs, so the rest is untouched.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 stage-5 quests document adding a checkpoint (with an `on_respawn` hook)
/// and a stealth beat over the hello-world anchors.
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
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "set-checkpoint", "anchor": "anchor/exit", "on_respawn": [ { "type": "narrate", "text": "You gather yourself." } ] },
            { "type": "begin-stealth", "zones": [ { "anchor": "anchor/exit", "extent": [2, 1, 2] } ], "on_caught": [ { "type": "narrate", "text": "Spotted!" } ], "grace_ticks": 20 }
          ]
        },
        "on_complete": [
          { "type": "end-stealth" },
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
        geometry_brief: None,
        layout_graph: None,
    }
}

/// The v0.6 checkpoint + stealth verbs validate clean under `dsl_version 0.6.0`
/// (no reserved-feature diagnostic).
#[test]
fn v06_verbs_validate_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        !diags.iter().any(|d| d.code == "DW0141"),
        "v0.6 checkpoint/stealth verbs must validate under 0.6.0 (no DW0141): {diags:#?}"
    );
}

/// The same verbs under a pre-0.6 quests version are reserved → `DW0141`.
#[test]
fn v06_verbs_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "v0.6 checkpoint/stealth verbs must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}
