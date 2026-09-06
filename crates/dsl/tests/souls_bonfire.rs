//! spec-0016 §1 — the `bonfire` rest verb and wave `respawns_on_rest`.
//!
//! `bonfire{anchor, on_rest}` is the souls sibling of `set-checkpoint`: it places
//! a rest affordance rather than moving the respawn point outright. It validates
//! under `dsl_version 0.6.0`; its anchor
//! resolves like every other effect anchor (`DW0142`); its `on_rest` bundle is a
//! first-class nested effect list (l10n inventory, deep consumer checks); and a
//! wave declaring `respawns_on_rest` with no bonfire to fire it is a **loud**
//! defect (`DW0370`), never a silent no-op.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, l10n_inventory, parse_campaign};

/// A v0.6 quests document with a bonfire (with an `on_rest` narrate) and a wave
/// that re-seats on rest.
const QUESTS_V06: &str = r#"{
  "dsl_version": "0.19.0",
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

/// The hello-world classes doc with a `flask` kit entry spliced in (v0.8): a
/// bonfire campaign owes the party one, and a campaign that does not is `DW0476`.
fn classes_with_flask() -> String {
    let mut v: serde_json::Value =
        serde_json::from_str(&common::read_valid("classes.json")).unwrap();
    v["dsl_version"] = serde_json::json!("0.19.0");
    for class in v["content"]["classes"].as_array_mut().unwrap() {
        class["kit"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "item": "minecraft:bread", "count": 3, "flask": true
            }));
    }
    serde_json::to_string(&v).unwrap()
}

fn campaign_with_quests(quests: &str) -> RawCampaign {
    campaign_with(quests, &classes_with_flask())
}

fn campaign_with(quests: &str, classes: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: classes.to_string(),
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

/// The whole spec-0016 §1 surface validates clean under 0.6.0.
#[test]
fn bonfire_and_rest_reseat_validate_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 bonfire campaign, got: {diags:#?}"
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

// ---------------------------------------------------------------------------
// The flask + the rest dialog
// ---------------------------------------------------------------------------

/// **A souls campaign whose kit declares no flask is a build error.** (Owner
/// ruling, verbatim.) The rest option's whole recovery half is a no-op without
/// one, so the compiler refuses rather than shipping a bonfire that only saves.
#[test]
fn a_bonfire_campaign_without_a_flask_is_dw0490() {
    let diags = campaign_with(QUESTS_V06, &common::read_valid("classes.json"));
    let diags = check_campaign(&diags);
    let hit = diags
        .iter()
        .find(|d| d.code == "DW0476")
        .unwrap_or_else(|| panic!("a flaskless bonfire campaign must be DW0476: {diags:#?}"));
    assert_eq!(hit.stage, "classes");
    assert!(
        hit.message.contains("class/warden") || hit.message.contains("class/"),
        "the message names the offending class: {}",
        hit.message
    );
}

/// …and a campaign with **no** bonfire is untouched by the rule: the flask is a
/// souls-mode obligation, not a universal one. Wave campaigns keep building.
#[test]
fn a_campaign_without_a_bonfire_needs_no_flask() {
    let no_bonfire = QUESTS_V06
        .replace("\"bonfire\"", "\"set-checkpoint\"")
        .replace("\"on_rest\"", "\"on_respawn\"")
        .replace("\"respawns_on_rest\": true,", "");
    let diags = check_campaign(&campaign_with(
        &no_bonfire,
        &common::read_valid("classes.json"),
    ));
    assert!(
        !diags.iter().any(|d| d.code == "DW0476"),
        "no bonfire, no flask obligation: {diags:#?}"
    );
}

/// spec-0016 §1: a **`boss`-tier wave may not declare
/// `respawns_on_rest`**. Beating a stage boss is progress the fire may not undo,
/// so the two declarations contradict each other — `DW0499`, and nothing silent.
///
/// The `elite` tier is deliberately still allowed to re-seat: an elite is a
/// set-piece the party may legitimately re-run, and spec-0016 §1 only exempts
/// stage bosses.
#[test]
fn a_boss_wave_may_not_declare_respawns_on_rest_dw0489() {
    let boss = QUESTS_V06.replacen("\"0.6.0\"", "\"0.8.0\"", 1).replace(
        "\"respawns_on_rest\": true,",
        "\"respawns_on_rest\": true, \"tier\": \"boss\",",
    );
    assert_ne!(boss, QUESTS_V06, "the substitution must apply");
    let diags = check_campaign(&campaign_with_quests(&boss));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0499" && d.path.ends_with("/respawns_on_rest")),
        "a boss wave that respawns on rest must be DW0499: {diags:#?}"
    );

    // The same wave billed `elite` is legal: only the stage boss is exempt.
    let elite = boss.replace("\"tier\": \"boss\"", "\"tier\": \"elite\"");
    let diags = check_campaign(&campaign_with_quests(&elite));
    assert!(
        diags.is_empty(),
        "an elite wave may still respawn on rest: {diags:#?}"
    );
}
