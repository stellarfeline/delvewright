//! spec-0021: the stage-5 `loot` section and actor `equipment`.
//!
//! Both are v0.6 surfaces, reserved (`DW0141`) earlier. Loot validates its item
//! ids against the pinned registry (`DW0143`), its anchors against prefab
//! metadata (`DW0142`), rejects two fills of one container (`DW0435`) and a
//! fill deeper than a container (`DW0432`). Enchantments — on loot stacks and
//! on equipped pieces alike — validate their id (`DW0433`) and level
//! (`DW0434`).
//!
//! The container-ness of the anchor's cell needs the assembled world, so it is
//! a build-tier proof (`DW0431`) covered in the compiler crate.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 `quests` document with pluggable `loot` and `actors` sections, over
/// the unchanged hello-world expansion.
const QUESTS_TMPL: &str = r#"{
  "dsl_version": "{VER}",
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
    "actors": [ {ACTORS} ],
    "loot": [ {LOOT} ]
  }
}"#;

fn campaign(version: &str, loot: &str, actors: &str) -> RawCampaign {
    let quests = QUESTS_TMPL
        .replacen("{VER}", version, 1)
        .replacen("{ACTORS}", actors, 1)
        .replacen("{LOOT}", loot, 1);
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests,
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
    geometry_brief: None,
    layout_graph: None,
}

fn with_loot(version: &str, loot: &str) -> RawCampaign {
    campaign(version, loot, "")
    geometry_brief: None,
    layout_graph: None,
}

fn with_actor(version: &str, actor: &str) -> RawCampaign {
    campaign(version, "", actor)
    geometry_brief: None,
    layout_graph: None,
}

const VALID_LOOT: &str = r#"{
  "id": "loot/galley-stores",
  "anchor": "anchor/exit",
  "items": [
    { "item": "minecraft:cooked_beef", "count": 3 },
    { "item": "minecraft:torch", "count": 4, "name": "Tide Lantern" }
  ]
}"#;

const VALID_ACTOR: &str = r#"{
  "id": "actor/elite",
  "entity": "minecraft:wither_skeleton",
  "anchor": "anchor/keeper-stand",
  "equipment": {
    "head": { "item": "minecraft:leather_boots",
              "enchantments": { "minecraft:protection": 4 } },
    "chest": "minecraft:leather_boots",
    "main_hand": { "item": "minecraft:iron_sword",
                   "enchantments": { "minecraft:sharpness": 5 } }
  }
}"#;

fn codes(c: &RawCampaign) -> Vec<String> {
    check_campaign(c).iter().map(|d| d.code.clone()).collect()
}

// --- the happy paths ------------------------------------------------------

#[test]
fn valid_loot_validates_clean() {
    let diags = check_campaign(&with_loot("0.6.0", VALID_LOOT));
    assert!(diags.is_empty(), "valid loot must be clean: {diags:#?}");
}

#[test]
fn a_fully_enchanted_actor_validates_clean() {
    let diags = check_campaign(&with_actor("0.6.0", VALID_ACTOR));
    assert!(
        diags.is_empty(),
        "valid actor gear must be clean: {diags:#?}"
    );
}

/// The plain-string form and the enchanted-object form coexist in one block.
#[test]
fn plain_and_enchanted_slots_coexist() {
    assert!(check_campaign(&with_actor("0.6.0", VALID_ACTOR)).is_empty());
}

// --- reservation ----------------------------------------------------------

#[test]
fn loot_is_reserved_before_0_6() {
    assert!(
        codes(&with_loot("0.5.0", VALID_LOOT)).contains(&"DW0141".to_string()),
        "the `loot` section must be reserved under 0.5.0"
    );
}

#[test]
fn actor_equipment_is_reserved_before_0_6() {
    assert!(
        codes(&with_actor("0.5.0", VALID_ACTOR)).contains(&"DW0141".to_string()),
        "actor `equipment` must be reserved under 0.5.0"
    );
}

// --- loot rules -----------------------------------------------------------

#[test]
fn an_unknown_loot_item_is_dw0143() {
    let l = VALID_LOOT.replacen("minecraft:cooked_beef", "minecraft:tide_biscuit", 1);
    assert!(codes(&with_loot("0.6.0", &l)).contains(&"DW0143".to_string()));
}

#[test]
fn a_loot_anchor_no_prefab_provides_is_dw0142() {
    let l = VALID_LOOT.replacen("anchor/exit", "anchor/nowhere", 1);
    assert!(codes(&with_loot("0.6.0", &l)).contains(&"DW0142".to_string()));
}

/// Two fills of one container: the second overwrites the first slot-for-slot,
/// so the first declaration's items would silently never appear.
#[test]
fn two_loot_entries_on_one_anchor_is_dw0435() {
    let second = VALID_LOOT.replacen("loot/galley-stores", "loot/galley-stores-2", 1);
    let both = format!("{VALID_LOOT}, {second}");
    assert!(codes(&with_loot("0.6.0", &both)).contains(&"DW0435".to_string()));
}

#[test]
fn more_stacks_than_a_container_has_slots_is_dw0432() {
    let items = (0..28)
        .map(|_| r#"{ "item": "minecraft:bread" }"#)
        .collect::<Vec<_>>()
        .join(",");
    let l = format!(r#"{{ "id": "loot/too-much", "anchor": "anchor/exit", "items": [{items}] }}"#);
    assert!(codes(&with_loot("0.6.0", &l)).contains(&"DW0432".to_string()));
}

// --- enchantment rules (shared by loot and equipment) ---------------------

#[test]
fn an_unknown_enchantment_id_is_dw0433() {
    let a = VALID_ACTOR.replacen("minecraft:sharpness", "minecraft:sharpness_v", 1);
    assert!(codes(&with_actor("0.6.0", &a)).contains(&"DW0433".to_string()));
}

/// The curse ids are the classic trap: vanilla calls them `binding_curse` and
/// `vanishing_curse`, never `curse_of_binding`.
#[test]
fn the_curse_id_trap_is_dw0433_and_the_real_id_is_clean() {
    let bad = VALID_ACTOR.replacen("minecraft:protection", "minecraft:curse_of_binding", 1);
    assert!(codes(&with_actor("0.6.0", &bad)).contains(&"DW0433".to_string()));
    let good = VALID_ACTOR.replacen("minecraft:protection", "minecraft:binding_curse", 1);
    assert!(check_campaign(&with_actor("0.6.0", &good)).is_empty());
}

#[test]
fn a_zero_enchantment_level_is_dw0434() {
    let a = VALID_ACTOR.replacen(
        r#""minecraft:sharpness": 5"#,
        r#""minecraft:sharpness": 0"#,
        1,
    );
    assert!(codes(&with_actor("0.6.0", &a)).contains(&"DW0434".to_string()));
}

#[test]
fn an_over_255_enchantment_level_is_dw0434() {
    let a = VALID_ACTOR.replacen(
        r#""minecraft:sharpness": 5"#,
        r#""minecraft:sharpness": 300"#,
        1,
    );
    assert!(codes(&with_actor("0.6.0", &a)).contains(&"DW0434".to_string()));
}

/// Above the SURVIVAL maximum is legal — that is how a set-piece elite is
/// built, and the compiler must not overrule the design.
#[test]
fn a_level_above_the_survival_max_is_allowed() {
    let a = VALID_ACTOR.replacen(
        r#""minecraft:sharpness": 5"#,
        r#""minecraft:sharpness": 10"#,
        1,
    );
    let diags = check_campaign(&with_actor("0.6.0", &a));
    assert!(diags.is_empty(), "sharpness 10 must be allowed: {diags:#?}");
}

/// Enchantments on a loot stack go through the same rule as equipped gear.
#[test]
fn loot_enchantments_share_the_equipment_rules() {
    let l = r#"{ "id": "loot/cache", "anchor": "anchor/exit",
                 "items": [ { "item": "minecraft:iron_sword",
                              "enchantments": { "minecraft:not_a_thing": 1 } } ] }"#;
    assert!(codes(&with_loot("0.6.0", l)).contains(&"DW0433".to_string()));
}
