//! DSL v0.6 `damage-players` (spec-0014): the stealth/souls consequence verb.
//! Validates under `dsl_version 0.6.0`, reserved (`DW0141`) earlier; an unknown
//! `damage_type` is a schema rejection (`DW0100`); an `in` filter-zone anchor the
//! prefab does not provide is `DW0142`; per-effect `requires_flags` is allowed
//! (it is a per-`@s` verb) and resolves against declared flags (`DW0172`).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 quests document that damages the party (lethal, generic) on the exit
/// beat — the "consequence" the verb exists for.
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
          { "type": "damage-players", "amount": 40, "damage_type": "wither" },
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

/// `damage-players` (with a curated `damage_type`) validates clean under 0.6.0.
#[test]
fn damage_players_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 damage-players, got: {diags:#?}"
    );
}

/// `damage-players` under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn damage_players_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "damage-players must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// An unknown `damage_type` enum value is a schema rejection (`DW0100`) — the
/// curated enum needs no separate registry/diagnostic.
#[test]
fn unknown_damage_type_is_dw0100() {
    let bad = QUESTS_V06.replace("\"wither\"", "\"void\"");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0100"),
        "an unknown damage_type must be a schema rejection (DW0100): {diags:#?}"
    );
}

/// A `damage-players` `in` anchor the prefab does not provide is `DW0142`.
#[test]
fn damage_players_unresolved_in_anchor_is_dw0142() {
    let scoped = QUESTS_V06.replace(
        r#"{ "type": "damage-players", "amount": 40, "damage_type": "wither" }"#,
        r#"{ "type": "damage-players", "amount": 6,
             "in": { "anchor": "anchor/nope", "extent": [2, 2, 2] } }"#,
    );
    let diags = check_campaign(&campaign_with_quests(&scoped));
    assert!(
        diags.iter().any(|d| d.code == "DW0142"),
        "an unresolved damage-players `in` anchor must be DW0142: {diags:#?}"
    );
}

/// `damage-players` accepts a per-effect `requires_flags` (it is a per-`@s` verb,
/// unlike the party/session-global checkpoint/stealth verbs); the flag must resolve
/// to a `set-flag` producer (`DW0172`).
#[test]
fn damage_players_requires_flags_resolves() {
    // Gate the damage on a flag set earlier in the same quest — validates clean.
    let gated = QUESTS_V06.replace(
        r#""obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]"#,
        r#""obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" },
                         { "type": "set-flag", "flag": "flag/doomed" } ]"#,
    );
    let gated = gated.replace(
        r#"{ "type": "damage-players", "amount": 40, "damage_type": "wither" }"#,
        r#"{ "type": "damage-players", "amount": 40, "damage_type": "wither",
             "requires_flags": ["flag/doomed"] }"#,
    );
    let diags = check_campaign(&campaign_with_quests(&gated));
    assert!(
        diags.is_empty(),
        "damage-players with a resolvable requires_flags must validate clean: {diags:#?}"
    );

    // An unresolved flag is DW0172.
    let bad = gated.replace("\"flag/doomed\"]", "\"flag/nope\"]");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0172"),
        "an unresolved damage-players requires_flags must be DW0172: {diags:#?}"
    );
}
