//! DSL v0.36 (spec-0092): a lightning bolt strikes at a mark.
//!
//! What this file pins down:
//! * the surface validates clean, and the exported schema carries the verb with
//!   one field of its own, `at` (acceptance criterion 1; the union's size is
//!   stated once, by `v29_firework`);
//! * the verb is a party fact: `audience` on it is `DW0942` (criterion 6);
//! * **the pinned facts** — the entity, the reach box, the damage and the burn,
//!   the fire gamerule and the blocks the bolt rewrites — so a re-pin is one
//!   diff in `crates/dsl/src/lightning.rs` and this file.

mod common;

use delvewright_dsl::envelope::Stage;
use delvewright_dsl::{DSL_VERSION, RawCampaign, check_campaign, lightning, stage_schema};
use serde_json::Value;

fn campaign_with(quests: &str) -> RawCampaign {
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
        design: None,
    }
}

/// A quests document whose single objective-completion bundle is `effects`.
fn quests_with(effects: &str) -> String {
    format!(
        r##"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }}
        ],
        "on_objective_complete": {{ "obj/talk": {effects} }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"##
    )
}

fn codes(quests: &str) -> Vec<String> {
    check_campaign(&campaign_with(quests))
        .into_iter()
        .map(|d| d.code)
        .collect()
}

fn verb_branch(schema: &Value, verb: &str) -> Value {
    schema["$defs"]["QuestEffect"]["oneOf"]
        .as_array()
        .expect("the effect is a tagged union of verb branches")
        .iter()
        .find(|b| b["properties"]["type"]["const"].as_str() == Some(verb))
        .unwrap_or_else(|| panic!("no schema branch declares `\"type\": \"{verb}\"`"))
        .clone()
}

fn keys(branch: &Value) -> Vec<String> {
    let mut k: Vec<String> = branch["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    k.sort();
    k
}

#[test]
fn a_strike_at_a_mark_validates_clean() {
    let q = quests_with(
        r#"[{ "type": "lightning", "at": { "anchor": "anchor/exit", "offset": [0, 5, 0] } }]"#,
    );
    let c = codes(&q);
    assert!(c.is_empty(), "a strike at a mark is a legal effect: {c:?}");
}

/// The verb's own fields are the ones a verb with none (`end-stealth`) does not
/// carry — the envelope every verb shares is taken away, and what is left is
/// `at`, required.
#[test]
fn the_schema_exports_the_verb_with_one_field_of_its_own() {
    let schema = stage_schema(Stage::Quests);
    let strike = verb_branch(&schema, "lightning");
    let bare = keys(&verb_branch(&schema, "end-stealth"));
    let own: Vec<String> = keys(&strike)
        .into_iter()
        .filter(|k| !bare.contains(k))
        .collect();
    assert_eq!(own, vec!["at".to_string()], "the strike's own fields");
    let required: Vec<&str> = strike["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(required.contains(&"at"), "a strike names where it lands");
    assert_eq!(
        strike["properties"]["at"]["$ref"].as_str(),
        Some("#/$defs/Mark"),
        "the place is a mark (spec-0066)"
    );
}

/// The thunder is the game's to send; the bolt is a world fact. Narrowing who
/// it addresses is refused, as on `firework`.
#[test]
fn an_audience_on_a_strike_is_dw0942() {
    let q = quests_with(
        r#"[{ "type": "lightning", "audience": "party", "at": { "anchor": "anchor/exit" } }]"#,
    );
    assert!(
        codes(&q).contains(&"DW0942".to_string()),
        "a party fact refuses an audience"
    );
}

#[test]
fn the_pinned_facts_are_what_the_bytes_say() {
    assert_eq!(lightning::BOLT_ENTITY, "minecraft:lightning_bolt");
    assert_eq!(
        (
            lightning::REACH_HORIZONTAL,
            lightning::REACH_BELOW,
            lightning::REACH_ABOVE
        ),
        (3, 3, 9),
        "`LightningBolt.tick` inflates by 3.0, and by 6.0 + 3.0 upward"
    );
    assert_eq!(
        (lightning::DAMAGE_HP, lightning::BURN_SECONDS),
        (5, 8),
        "`Entity.thunderHit`: 5.0F of lightning_bolt, igniteForSeconds(8.0F)"
    );
    assert_eq!(lightning::worst_damage_hp(), 13);
    assert_eq!(lightning::FIRE_GAMERULE, "fire_spread_radius_around_player");
    assert_eq!(
        lightning::READ_FROM.len(),
        4,
        "the classes a re-pin re-reads"
    );
    assert!(lightning::rewrites(
        "minecraft:lightning_rod[facing=up,powered=false,waterlogged=false]"
    ));
    assert!(lightning::rewrites("minecraft:waxed_copper_block"));
    assert!(!lightning::rewrites("minecraft:stone"));
}
