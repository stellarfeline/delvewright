//! spec-0016 §3 — the stage-5 `ambushes` section.
//!
//! An ambush is **sugar**: one declaration for what is otherwise a deferred
//! actor set plus a hand-wired trigger. `parse_campaign` desugars it into a real
//! `EnvTrigger` at the DSL boundary, so every downstream consumer — validation,
//! l10n, the flag/wave producer scans, nav, emission — sees the trigger it always
//! saw and the sugar has no second code path to drift down.
//!
//! `telegraph` is **optional and stays optional**: the un-telegraphed ambush is
//! core souls vocabulary (owner ruling 2026-08-02). The declaration checks here
//! (`DW0375`) never ask for one. What the engine owes the player is counterplay
//! on the retry, which is geometric and lives in `compiler::nav` (`DW0376`).

mod common;

use delvewright_dsl::{RawCampaign, TriggerOn, check_campaign, l10n_inventory, parse_campaign};

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
        "on_objective_complete": { "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ] },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "actors": [
      { "id": "actor/lurker", "entity": "minecraft:husk", "anchor": "anchor/keeper-stand" }
    ],
    "ambushes": [
      {
        "id": "ambush/door-turn",
        "at": "anchor/exit",
        "actors": ["actor/lurker"],
        "trigger": { "on": "approach", "range": 3 }
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

/// An un-telegraphed ambush validates clean. This is the owner ruling encoded as
/// a test: the engine never demands a tell.
#[test]
fn untelegraphed_ambush_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "an un-telegraphed ambush is legitimate souls vocabulary: {diags:#?}"
    );
}

/// The sugar expands to exactly the trigger an author would hand-write: one-shot,
/// at the declared anchor, telegraph first, then a `spawn-actor` and an
/// `unleash-actor` per ambusher.
#[test]
fn ambush_desugars_to_a_one_shot_trigger() {
    let telegraphed = QUESTS_V06.replace(
        "\"trigger\": { \"on\": \"approach\", \"range\": 3 }",
        "\"trigger\": { \"on\": \"approach\", \"range\": 3 },\n        \"telegraph\": [ { \"type\": \"narrate\", \"text\": \"Gravel shifts behind you.\", \"style\": \"subtitle\" } ]",
    );
    assert_ne!(telegraphed, QUESTS_V06);
    let campaign = parse_campaign(&campaign_with_quests(&telegraphed)).expect("parses");
    let trig = campaign
        .quests
        .content
        .triggers
        .iter()
        .find(|t| t.id.as_str() == "trigger/door-turn")
        .expect("the ambush desugared to a trigger named after itself");
    assert!(trig.once, "an ambush springs once");
    assert_eq!(trig.at_anchor(), Some("anchor/exit"));
    assert!(matches!(trig.on, TriggerOn::Approach { range: 3 }));
    let verbs: Vec<&str> = trig.effects.iter().map(|e| e.verb()).collect();
    assert_eq!(
        verbs,
        vec!["narrate", "spawn-actor", "unleash-actor"],
        "telegraph first, then spawn, then unleash"
    );
    // Expansion is idempotent: the authored list is retained for diagnostics but
    // a second expansion must not duplicate the trigger.
    let mut again = campaign.clone();
    again.quests.content.expand_ambushes();
    assert_eq!(
        again.quests.content.triggers.len(),
        campaign.quests.content.triggers.len(),
        "expansion is idempotent"
    );
}

/// A telegraph string is player-visible text, so it must be translatable — the
/// expansion puts it on the ordinary trigger path, which the l10n inventory
/// already walks.
#[test]
fn telegraph_strings_enter_the_l10n_inventory() {
    let telegraphed = QUESTS_V06.replace(
        "\"trigger\": { \"on\": \"approach\", \"range\": 3 }",
        "\"trigger\": { \"on\": \"approach\", \"range\": 3 },\n        \"telegraph\": [ { \"type\": \"narrate\", \"text\": \"Gravel shifts behind you.\", \"style\": \"subtitle\" } ]",
    );
    let campaign = parse_campaign(&campaign_with_quests(&telegraphed)).expect("parses");
    let inv = l10n_inventory(&campaign);
    assert!(
        inv.values().any(|v| v == "Gravel shifts behind you."),
        "the telegraph line must be translatable: {inv:#?}"
    );
}

/// The `ambushes` section under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn ambushes_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0141" && d.path == "/content/ambushes"),
        "the ambushes section must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// An ambush that lists no actors springs nothing — `DW0375`, never a silent
/// no-op.
#[test]
fn ambush_with_no_actors_is_dw0375() {
    let bad = QUESTS_V06.replace("\"actors\": [\"actor/lurker\"]", "\"actors\": []");
    assert_ne!(bad, QUESTS_V06);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0375"),
        "an actor-less ambush must be DW0375: {diags:#?}"
    );
}

/// The same actor listed twice reads as a pair but stages one body —
/// `spawn-actor` is idempotent. `DW0375`.
#[test]
fn ambush_listing_an_actor_twice_is_dw0375() {
    let bad = QUESTS_V06.replace(
        "\"actors\": [\"actor/lurker\"]",
        "\"actors\": [\"actor/lurker\", \"actor/lurker\"]",
    );
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0375"),
        "a doubled actor must be DW0375: {diags:#?}"
    );
}

/// A malformed ambush id is `DW0375`.
#[test]
fn malformed_ambush_id_is_dw0375() {
    let bad = QUESTS_V06.replace("\"ambush/door-turn\"", "\"Door Turn\"");
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0375"),
        "a malformed ambush id must be DW0375: {diags:#?}"
    );
}
