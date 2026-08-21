//! DSL v0.6 `world.difficulty` + actor `attributes`.
//!
//! Difficulty was a compiler constant: `easy` for any campaign with a wave,
//! `peaceful` for one without. Easy halves incoming player damage
//! (`min(dmg / 2 + 1, dmg)`), so "the enemies are too weak" was in part a setting
//! nobody could declare. It is now a stage-1 field — an additive v0.6 surface,
//! reserved (`DW0141`) on an older world stage, absent by default so every
//! earlier campaign keeps the derivation.
//!
//! `peaceful` is refused (`DW0468`): on peaceful the server discards every
//! hostile-category mob as it ticks it, so the delve's whole cast of threats
//! would silently not exist. The mirror case — actors but no waves and no
//! declared difficulty, i.e. the derived `peaceful` deleting them — is the
//! advisory `DW0469`.
//!
//! Actor `attributes` is the same v0.4 [`MobAttributes`] shape a wave mob takes,
//! fenced on the stage actors themselves are fenced on.

mod common;

use delvewright_dsl::{RawCampaign, Severity, check_campaign};

fn raw_with(world: Option<&str>, quests: Option<&str>) -> RawCampaign {
    RawCampaign {
        world: world
            .map(str::to_string)
            .unwrap_or_else(|| common::read_valid("world.json")),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests
            .map(str::to_string)
            .unwrap_or_else(|| common::read_valid("quests.json")),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
    geometry_brief: None,
    layout_graph: None,
}

/// hello-world's world stage at `version` with a declared `difficulty`.
fn world_with_difficulty(value: &str, version: &str) -> String {
    common::read_valid("world.json")
        .replacen("\"0.2.0\"", &format!("\"{version}\""), 1)
        .replacen(
            "\"target_minutes\": 5,",
            &format!("\"target_minutes\": 5,\n    \"difficulty\": \"{value}\","),
            1,
        )
}

/// A v0.6 quests document with one scripted actor, spawned and unleashed from the
/// quest's first beat. `attrs` is spliced into the actor object (`""` for none).
fn quests_with_actor(attrs: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "spawn-actor", "actor": "actor/giant" }},
            {{ "type": "unleash-actor", "actor": "actor/giant" }}
          ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "actors": [
      {{ "id": "actor/giant", "entity": "minecraft:zombie", "name": "The Sleeper",
         "anchor": "anchor/keeper-stand"{attrs} }}
    ]
  }}
}}"#
    )
}

fn codes(raw: &RawCampaign) -> Vec<String> {
    check_campaign(raw).iter().map(|d| d.code.clone()).collect()
}

// --- the field itself ---------------------------------------------------------

/// The three legal keywords all validate clean on a v0.6 world stage.
#[test]
fn easy_normal_hard_all_validate() {
    for value in ["easy", "normal", "hard"] {
        let raw = raw_with(Some(&world_with_difficulty(value, "0.6.0")), None);
        let d = check_campaign(&raw);
        assert!(d.is_empty(), "`{value}` must validate clean: {d:#?}");
    }
}

/// `peaceful` is refused with `DW0468`, and the message must carry the *reason*
/// (hostiles are discarded) — the whole point of spending a code on it rather
/// than letting the schema emit "unknown variant".
#[test]
fn peaceful_is_rejected_with_its_rationale() {
    let raw = raw_with(Some(&world_with_difficulty("peaceful", "0.6.0")), None);
    let d = check_campaign(&raw);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0468")
        .expect("peaceful raises DW0468");
    assert_eq!(hit.severity, Severity::Error);
    assert_eq!(hit.stage, "world");
    assert_eq!(hit.path, "/content/difficulty");
    assert!(
        hit.message.contains("discards every")
            && hit.message.contains("hostile")
            && hit.message.contains("ambush"),
        "the diagnostic must explain that every wave/actor/ambush would vanish: {}",
        hit.message
    );
    assert!(
        hit.message.contains("easy") && hit.message.contains("hard"),
        "the diagnostic must prescribe the legal keywords: {}",
        hit.message
    );
}

/// The field is an additive v0.6 surface: declaring it on an older world stage
/// is `DW0141`, exactly like `horizon` / `boundary` / `min_players`.
#[test]
fn difficulty_is_reserved_before_v06() {
    for version in ["0.2.0", "0.5.0"] {
        let raw = raw_with(Some(&world_with_difficulty("hard", version)), None);
        let d = check_campaign(&raw);
        let hit = d
            .iter()
            .find(|x| x.code == "DW0141" && x.path == "/content/difficulty")
            .unwrap_or_else(|| panic!("difficulty under {version} must be reserved: {d:#?}"));
        assert!(hit.message.contains("0.6.0"), "{}", hit.message);
    }
}

/// A campaign that declares nothing is unchanged — no diagnostic, no field.
#[test]
fn absent_difficulty_is_clean() {
    assert!(check_campaign(&raw_with(None, None)).is_empty());
}

// --- DW0469: the derived peaceful deleting scripted actors ---------------------

/// Actors, no waves, no declared difficulty ⇒ the build ships the derived
/// `peaceful`, which discards every hostile-species actor. Advisory, because the
/// pinned entity registry is a membership set and cannot tell the compiler
/// whether `actor/giant` is a monster.
#[test]
fn actors_without_waves_or_declared_difficulty_warn() {
    let raw = raw_with(None, Some(&quests_with_actor("")));
    let d = check_campaign(&raw);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0469")
        .unwrap_or_else(|| panic!("actors + derived peaceful must warn: {d:#?}"));
    assert_eq!(hit.severity, Severity::Warning);
    assert_eq!(hit.path, "/content/difficulty");
    assert!(
        hit.message.contains("peaceful"),
        "the warning must name the setting it is about: {}",
        hit.message
    );
    // Advisory only: nothing here is an error, so the campaign still builds.
    assert!(
        d.iter().all(|x| x.severity == Severity::Warning),
        "DW0469 must not fail the run: {d:#?}"
    );
}

/// Declaring a difficulty settles the question — the warning is gone.
#[test]
fn declared_difficulty_silences_the_actor_warning() {
    let raw = raw_with(
        Some(&world_with_difficulty("normal", "0.6.0")),
        Some(&quests_with_actor("")),
    );
    assert!(
        !codes(&raw).contains(&"DW0469".to_string()),
        "a declared difficulty answers the question DW0469 asks"
    );
}

/// A campaign with no actors never sees the warning, whatever its difficulty.
#[test]
fn no_actors_means_no_warning() {
    assert!(!codes(&raw_with(None, None)).contains(&"DW0469".to_string()));
}

// --- actor attributes ---------------------------------------------------------

/// Actor `attributes` takes the wave-mob shape and validates clean under v0.6.
#[test]
fn actor_attributes_validate_under_v06() {
    let attrs =
        r#", "attributes": { "max_health": 200.0, "attack_damage": 12.0, "follow_range": 24.0 }"#;
    let raw = raw_with(
        Some(&world_with_difficulty("hard", "0.6.0")),
        Some(&quests_with_actor(attrs)),
    );
    let d = check_campaign(&raw);
    assert!(d.is_empty(), "actor attributes must validate clean: {d:#?}");
}

/// ...and are reserved (`DW0141`) on a pre-0.6 quests stage, like actor
/// `equipment` — the surface they extend.
#[test]
fn actor_attributes_are_reserved_before_v06() {
    let attrs = r#", "attributes": { "max_health": 200.0 }"#;
    let quests = quests_with_actor(attrs).replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let raw = raw_with(None, Some(&quests));
    let d = check_campaign(&raw);
    assert!(
        d.iter()
            .any(|x| x.code == "DW0141" && x.path == "/content/actors/0/attributes"),
        "actor attributes under 0.5 must be reserved: {d:#?}"
    );
}

/// An unknown attribute name is a schema rejection, not a silently-ignored field
/// — the shape is `deny_unknown_fields`, shared with the wave-mob surface.
#[test]
fn unknown_actor_attribute_is_a_schema_error() {
    let attrs = r#", "attributes": { "armor_toughness": 8.0 }"#;
    let raw = raw_with(
        Some(&world_with_difficulty("hard", "0.6.0")),
        Some(&quests_with_actor(attrs)),
    );
    assert!(
        codes(&raw).contains(&"DW0100".to_string()),
        "an attribute the DSL does not expose must be rejected, not dropped"
    );
}
