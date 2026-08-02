//! spec-0016 §1 — the `bonfire` rest verb and wave `respawns_on_rest`.
//!
//! `bonfire{anchor, on_rest}` is the souls sibling of `set-checkpoint`: it places
//! a rest affordance rather than moving the respawn point outright. It validates
//! under `dsl_version 0.6.0` and is reserved (`DW0141`) earlier; its anchor
//! resolves like every other effect anchor (`DW0142`); its `on_rest` bundle is a
//! first-class nested effect list (l10n inventory, deep consumer checks); and a
//! wave declaring `respawns_on_rest` with no bonfire to fire it is a **loud**
//! defect (`DW0370`), never a silent no-op.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, l10n_inventory, parse_campaign};

/// A v0.6 quests document with a bonfire (with an `on_rest` narrate) and a wave
/// that re-seats on rest.
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
          { "type": "kill", "id": "obj/slay", "wave": "wave/ambush", "after": ["obj/talk"] },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/slay"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "spawn-wave", "wave": "wave/ambush" },
            { "type": "bonfire", "anchor": "anchor/keeper-stand",
              "on_rest": [ { "type": "narrate", "text": "The fire steadies you.", "style": "chat" } ] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "waves": [
      {
        "id": "wave/ambush",
        "anchor": "anchor/keeper-stand",
        "respawns_on_rest": true,
        "mobs": [ { "entity": "minecraft:zombie", "count": 1 } ]
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

/// The whole spec-0016 §1 surface validates clean under 0.6.0.
#[test]
fn bonfire_and_rest_reseat_validate_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 bonfire campaign, got: {diags:#?}"
    );
}

/// `bonfire` under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn bonfire_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "the bonfire verb must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// A bonfire anchor no prefab exposes is `DW0142`, exactly like `set-checkpoint`
/// — a rest point the compiler cannot place is never silently dropped.
#[test]
fn bonfire_unknown_anchor_is_dw0142() {
    let bad = QUESTS_V06.replace(
        "\"anchor\": \"anchor/keeper-stand\",\n              \"on_rest\"",
        "\"anchor\": \"anchor/invented\",\n              \"on_rest\"",
    );
    assert_ne!(bad, QUESTS_V06, "the substitution must apply");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0142"),
        "an invented bonfire anchor must be DW0142: {diags:#?}"
    );
}

/// A wave declaring `respawns_on_rest` in a campaign with no `bonfire` is inert
/// — nothing could ever re-seat it. That is `DW0370`, not a silent no-op.
#[test]
fn respawns_on_rest_without_a_bonfire_is_dw0356() {
    let no_bonfire = QUESTS_V06
        .replace("\"bonfire\"", "\"set-checkpoint\"")
        .replace("\"on_rest\"", "\"on_respawn\"");
    assert_ne!(no_bonfire, QUESTS_V06, "the substitution must apply");
    let diags = check_campaign(&campaign_with_quests(&no_bonfire));
    assert!(
        diags.iter().any(|d| d.code == "DW0370"),
        "an unreachable respawns_on_rest must be DW0370: {diags:#?}"
    );
}

/// `respawns_on_rest` under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn respawns_on_rest_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0141" && d.path.ends_with("respawns_on_rest")),
        "wave `respawns_on_rest` must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// An `on_rest` narrate enters the l10n inventory under the `rest` nesting
/// segment — nested player-visible text is never left untranslatable, and the
/// key is deterministic (ADR-0006).
#[test]
fn on_rest_strings_enter_the_l10n_inventory() {
    let campaign = parse_campaign(&campaign_with_quests(QUESTS_V06)).expect("parses");
    let inv = l10n_inventory(&campaign);
    let key = inv
        .iter()
        .find(|(_, v)| v.as_str() == "The fire steadies you.")
        .map(|(k, _)| k.clone())
        .unwrap_or_else(|| panic!("on_rest narrate missing from the inventory: {inv:#?}"));
    assert!(
        key.contains(".rest."),
        "the on_rest key must carry the `rest` nesting segment, got `{key}`"
    );
    assert_eq!(inv, l10n_inventory(&campaign), "inventory is deterministic");
}
