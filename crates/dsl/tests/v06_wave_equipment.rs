//! DSL v0.6 wave-mob `equipment`: optional worn/held gear on a
//! [`delvewright_dsl::WaveMob`] — the sanctioned daylight-undead fix is a
//! helmet, never `set-time`. Validates under `dsl_version 0.6.0`, reserved
//! (`DW0141`) earlier; slot item ids validate against the pinned 1.21.11 item
//! registry (`DW0143`, the give-item family); an unknown slot name is a schema
//! rejection (`DW0100`).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 quests document with a wave whose zombie wears boots and carries a
/// sword (both in the vendored v0 item-registry subset the DSL tests use).
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
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "waves": [
      {
        "id": "wave/ambush",
        "anchor": "anchor/keeper-stand",
        "mobs": [
          { "entity": "minecraft:zombie", "count": 1,
            "equipment": { "feet": "minecraft:leather_boots", "main_hand": "minecraft:iron_sword" } }
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

/// Wave-mob `equipment` with registry-known item ids validates clean under 0.6.0.
#[test]
fn wave_equipment_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 wave-mob equipment, got: {diags:#?}"
    );
}

/// `equipment` under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn wave_equipment_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "wave-mob equipment must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// A slot item id absent from the pinned item registry is `DW0143` — the same
/// mechanism and DW family as `give-item`.
#[test]
fn unknown_equipment_item_is_dw0143() {
    let bad = QUESTS_V06.replace("minecraft:leather_boots", "minecraft:not_a_real_item");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0143" && d.path.contains("/equipment/feet")),
        "an unknown equipment item must be DW0143 at the slot path: {diags:#?}"
    );
}

/// An unknown slot name inside `equipment` is a schema rejection (`DW0100`) —
/// the slot set is closed (head/chest/legs/feet/main_hand/off_hand).
#[test]
fn unknown_equipment_slot_is_dw0100() {
    let bad = QUESTS_V06.replace("\"feet\":", "\"tail\":");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0100"),
        "an unknown equipment slot must be a schema rejection (DW0100): {diags:#?}"
    );
}
