//! DSL v0.6 (spec-0014): stage-5 scripted `actors` and the actor staging effects
//! (`spawn-actor`/`despawn-actor`/`move-actor`/`unleash-actor`/`sequence`) validate
//! under `0.6.0` and are reserved (`DW0141`) earlier. A `sequence` nested inside a
//! `sequence` is `DW0329`; a staging effect referencing an undeclared actor is
//! `DW0112`; an unknown actor entity is `DW0173`; an actor anchor a prefab does not
//! provide is `DW0142`.
//!
//! Built on the hello-world casting/dialogue (unchanged) with a v0.6 stage-5
//! `quests` document — additive, so the rest is untouched.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 stage-5 quests document: one actor puppet, spawned then walked to the
/// exit (despawn-on-arrive), then a two-step timeline unleashing and killing it.
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
            { "type": "spawn-actor", "actor": "actor/giant" },
            { "type": "move-actor", "actor": "actor/giant", "to_anchor": "anchor/exit",
              "on_arrive": [ { "type": "despawn-actor", "actor": "actor/giant", "style": "vanish" } ] },
            { "type": "sequence", "steps": [
              { "at_ticks": 0, "effects": [ { "type": "unleash-actor", "actor": "actor/giant" } ] },
              { "at_ticks": 40, "effects": [ { "type": "despawn-actor", "actor": "actor/giant", "style": "kill" } ] }
            ] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "actors": [
      { "id": "actor/giant", "entity": "minecraft:zombie", "name": "The Sleeper",
        "anchor": "anchor/keeper-stand", "facing": "north" }
    ]
  }
}"#;

/// hello-world's world stage raised to 0.6.0 with a declared `difficulty`.
///
/// The actor surface under test **unleashes** its giant, and this campaign has no
/// `waves[]` — the exact shape `DW0469` warns about, because the derived
/// `difficulty=peaceful` would have the server discard the unleashed twin on the
/// tick it spawned. Declaring the difficulty is what makes the fixture a campaign
/// whose actor surface actually exists at runtime.
fn world_v06_normal() -> String {
    common::read_valid("world.json")
        .replacen("\"0.2.0\"", "\"0.6.0\"", 1)
        .replacen(
            "\"target_minutes\": 5,",
            "\"target_minutes\": 5,\n    \"difficulty\": \"normal\",",
            1,
        )
}

fn campaign_with_quests(quests: &str) -> RawCampaign {
    RawCampaign {
        world: world_v06_normal(),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
}

/// The full v0.6 actor surface validates clean under `dsl_version 0.6.0`.
#[test]
fn v06_actor_surface_validates_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.6 actor surface, got: {diags:#?}"
    );
}

/// The same surface under a pre-0.6 quests version is reserved → `DW0141`.
#[test]
fn v06_actor_surface_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_quests(&pre));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "v0.6 actor surface must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// A `sequence` nested inside a `sequence` step is `DW0329`.
#[test]
fn v06_nested_sequence_is_dw0329() {
    // Wrap the innermost `unleash-actor` in a nested sequence.
    let nested = QUESTS_V06.replacen(
        r#"{ "type": "unleash-actor", "actor": "actor/giant" }"#,
        r#"{ "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [ { "type": "unleash-actor", "actor": "actor/giant" } ] } ] }"#,
        1,
    );
    let diags = check_campaign(&campaign_with_quests(&nested));
    assert!(
        diags.iter().any(|d| d.code == "DW0329"),
        "a sequence nested in a sequence must be DW0329: {diags:#?}"
    );
}

/// A staging effect referencing an undeclared actor is `DW0112`.
#[test]
fn v06_dangling_actor_ref_is_dw0112() {
    // Rename the declared actor so every reference to `actor/giant` dangles.
    let dangling = QUESTS_V06.replacen(
        r#"{ "id": "actor/giant", "entity": "minecraft:zombie""#,
        r#"{ "id": "actor/other", "entity": "minecraft:zombie""#,
        1,
    );
    let diags = check_campaign(&campaign_with_quests(&dangling));
    assert!(
        diags.iter().any(|d| d.code == "DW0112"),
        "an effect referencing an undeclared actor must be DW0112: {diags:#?}"
    );
}

/// An unknown actor entity id is `DW0173`.
#[test]
fn v06_unknown_actor_entity_is_dw0173() {
    let bad = QUESTS_V06.replacen("minecraft:zombie", "minecraft:not_a_real_entity", 1);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0173"),
        "an unknown actor entity must be DW0173: {diags:#?}"
    );
}

/// An actor anchor no prefab provides is `DW0142`.
#[test]
fn v06_unresolved_actor_anchor_is_dw0142() {
    let bad = QUESTS_V06.replacen("anchor/keeper-stand", "anchor/nowhere", 1);
    let diags = check_campaign(&campaign_with_quests(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0142"),
        "an actor anchor no prefab provides must be DW0142: {diags:#?}"
    );
}

/// Duplicate actor ids are `DW0111`.
#[test]
fn v06_duplicate_actor_id_is_dw0111() {
    let dup = QUESTS_V06.replacen(
        r#""actors": [
      { "id": "actor/giant", "entity": "minecraft:zombie", "name": "The Sleeper",
        "anchor": "anchor/keeper-stand", "facing": "north" }
    ]"#,
        r#""actors": [
      { "id": "actor/giant", "entity": "minecraft:zombie", "anchor": "anchor/keeper-stand" },
      { "id": "actor/giant", "entity": "minecraft:zombie", "anchor": "anchor/exit" }
    ]"#,
        1,
    );
    let diags = check_campaign(&campaign_with_quests(&dup));
    assert!(
        diags.iter().any(|d| d.code == "DW0111"),
        "a duplicate actor id must be DW0111: {diags:#?}"
    );
}
