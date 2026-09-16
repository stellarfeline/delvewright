//! DSL v0.29 (spec-0068): a firework is an effect.
//!
//! What this file pins down:
//! * the surface validates clean, and the exported schema carries it — the
//!   verb, its mark, `flight`'s `1..=3`, `explosions`' `1..=7`, the explosion
//!   object's five fields and a `FireworkShape` of exactly five variants
//!   (acceptance criterion 1);
//! * the `DW0100` enumeration names thirty-eight verbs;
//! * a malformed colour is `DW0100`, and the schema's own pattern is the rule
//!   the validator applies (criterion 3);
//! * a flight outside the crafted range and an explosion list outside `1..=7`
//!   are `DW0100`;
//! * **the pinned facts** — the three wiki pages, the five shapes, the
//!   `LifeTime` formula's fixed term, the heights 8/18/32, the five-block
//!   radius and the `FireworksItem` key — are asserted here, so a re-pin is one
//!   diff in `crates/dsl/src/firework.rs` and this file (criterion 7).

mod common;

use delvewright_dsl::envelope::Stage;
use delvewright_dsl::{DSL_VERSION, RawCampaign, check_campaign, firework, stage_schema};
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

/// One firework of every shape, with a fade, a trail and a twinkle, validates
/// clean — the shape the gallery's valley-site court fires.
#[test]
fn the_firework_surface_validates_clean() {
    let q = quests_with(
        r##"[
      { "type": "firework",
        "at": { "anchor": "anchor/exit", "offset": [0, 0, 2] },
        "flight": 1,
        "explosions": [
          { "shape": "large_ball", "colors": ["#ffd700", "#ffffff"],
            "fade_colors": ["#8b0000"], "trail": true, "twinkle": true },
          { "shape": "small_ball", "colors": ["#4fa3ff"] },
          { "shape": "star", "colors": ["#ffe066"] },
          { "shape": "creeper", "colors": ["#5aa02c"] },
          { "shape": "burst", "colors": ["#ffffff"] }
        ] }
    ]"##,
    );
    let d = check_campaign(&campaign_with(&q));
    assert!(d.is_empty(), "expected clean, got: {d:#?}");
}

/// `flight` is optional and defaults to the shortest crafted duration.
#[test]
fn flight_is_optional() {
    let q = quests_with(
        r##"[
      { "type": "firework", "at": { "anchor": "anchor/exit" },
        "explosions": [ { "shape": "burst", "colors": ["#ffffff"] } ] }
    ]"##,
    );
    let d = check_campaign(&campaign_with(&q));
    assert!(d.is_empty(), "expected clean, got: {d:#?}");
}

/// A colour that is not a `#rrggbb` literal is `DW0100`, on either colour list —
/// criterion 3's second half. The message names the schema's own pattern, so the
/// document tier and the schema tier state one rule.
#[test]
fn a_malformed_colour_is_dw0100() {
    for colours in [
        r##""colors": ["ffd700"]"##,
        r##""colors": ["#fff"]"##,
        r##""colors": ["#gggggg"]"##,
        r##""colors": ["#ffd700"], "fade_colors": ["red"]"##,
    ] {
        let q = quests_with(&format!(
            r##"[{{ "type": "firework", "at": {{ "anchor": "anchor/exit" }},
                  "explosions": [ {{ "shape": "burst", {colours} }} ] }}]"##
        ));
        let d = check_campaign(&campaign_with(&q));
        assert!(
            d.iter().any(|x| x.code == "DW0100"),
            "malformed colour {colours} must be DW0100, got: {d:#?}"
        );
        assert!(
            d.iter()
                .any(|x| x.message.contains(delvewright_dsl::color::HEX_PATTERN)),
            "the refusal names the schema's own pattern for {colours}"
        );
    }
}

/// A flight the game does not craft, and an explosion list outside `1..=7`, are
/// each `DW0100`: the exported schema says so and serde does not enforce it, so
/// the document tier states the same bound.
#[test]
fn an_out_of_range_flight_or_burst_count_is_dw0100() {
    let one = r##"{ "shape": "burst", "colors": ["#ffffff"] }"##;
    let eight = (0..8).map(|_| one).collect::<Vec<_>>().join(",");
    for effect in [
        format!(
            r##"{{ "type": "firework", "at": {{ "anchor": "anchor/exit" }},
                  "flight": 0, "explosions": [{one}] }}"##
        ),
        format!(
            r##"{{ "type": "firework", "at": {{ "anchor": "anchor/exit" }},
                  "flight": 4, "explosions": [{one}] }}"##
        ),
        r##"{ "type": "firework", "at": { "anchor": "anchor/exit" },
              "explosions": [] }"##
            .to_string(),
        format!(
            r##"{{ "type": "firework", "at": {{ "anchor": "anchor/exit" }},
                  "explosions": [{eight}] }}"##
        ),
    ] {
        let q = quests_with(&format!("[{effect}]"));
        assert!(
            codes(&q).contains(&"DW0100".to_string()),
            "out-of-range firework must be DW0100: {effect}"
        );
    }
}

/// A star with no colour is `DW0100` — the `minItems: 1` the schema states.
#[test]
fn a_star_with_no_colour_is_dw0100() {
    let q = quests_with(
        r##"[{ "type": "firework", "at": { "anchor": "anchor/exit" },
              "explosions": [ { "shape": "burst", "colors": [] } ] }]"##,
    );
    assert!(codes(&q).contains(&"DW0100".to_string()));
}

/// An unknown shape is a schema rejection at parse time, because the shape is an
/// enum rather than a free string.
#[test]
fn an_unknown_shape_is_dw0100() {
    let q = quests_with(
        r##"[{ "type": "firework", "at": { "anchor": "anchor/exit" },
              "explosions": [ { "shape": "spiral", "colors": ["#ffffff"] } ] }]"##,
    );
    assert!(codes(&q).contains(&"DW0100".to_string()));
}

/// A mark whose anchor no area provides is the dangling reference every other
/// anchor-bearing effect gets — the firework adds no rule of its own for it.
#[test]
fn a_dangling_mark_is_the_ordinary_dangling_reference() {
    let q = quests_with(
        r##"[{ "type": "firework", "at": { "anchor": "anchor/nowhere" },
              "explosions": [ { "shape": "burst", "colors": ["#ffffff"] } ] }]"##,
    );
    let d = check_campaign(&campaign_with(&q));
    assert!(
        !d.is_empty() && d.iter().all(|x| x.code != "DW0899"),
        "a dangling mark is not the firework's own refusal: {d:#?}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 1 — the exported schema
// ---------------------------------------------------------------------------

/// The one-of branch of the quests schema's effect union that carries
/// `"type": "firework"`.
fn verb_branches(schema: &Value) -> Vec<Value> {
    schema["$defs"]["QuestEffect"]["oneOf"]
        .as_array()
        .expect("the effect is a tagged union of verb branches")
        .clone()
}

fn verb_tag(branch: &Value) -> Option<&str> {
    branch
        .get("properties")?
        .get("type")?
        .get("const")?
        .as_str()
}

fn firework_branch(schema: &Value) -> Value {
    verb_branches(schema)
        .into_iter()
        .find(|b| verb_tag(b) == Some("firework"))
        .expect("no schema branch declares `\"type\": \"firework\"`")
}

/// The verb, its fields and their bounds are all in the exported schema — the
/// document an agent writes against.
#[test]
fn the_schema_exports_the_verb_and_its_bounds() {
    let schema = stage_schema(Stage::Quests);
    let branch = firework_branch(&schema);
    let props = branch["properties"].as_object().unwrap();
    for field in ["at", "flight", "explosions"] {
        assert!(props.contains_key(field), "`{field}` is on the verb");
    }
    let required: Vec<&str> = branch["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert!(
        required.contains(&"at"),
        "a firework names where it is fired"
    );
    assert!(required.contains(&"explosions"), "a rocket owes a burst");
    assert!(
        !required.contains(&"flight"),
        "`flight` defaults to the shortest crafted duration"
    );

    let flight = &props["flight"];
    let flight = flight.to_string();
    assert!(
        flight.contains("\"minimum\":1") && flight.contains("\"maximum\":3"),
        "`flight` carries the crafted range: {flight}"
    );
    let ex = props["explosions"].to_string();
    assert!(
        ex.contains("\"minItems\":1") && ex.contains("\"maxItems\":7"),
        "`explosions` carries `1..=7`: {ex}"
    );
}

/// The explosion object's five fields, and the colour pattern the validator
/// applies — one rule, stated in one place and exported from it.
#[test]
fn the_schema_exports_the_explosion_and_its_colour_pattern() {
    let schema = stage_schema(Stage::Quests);
    let defs = schema["$defs"].as_object().unwrap();
    let ex = defs
        .get("FireworkExplosion")
        .expect("the explosion is its own schema definition");
    let props = ex["properties"].as_object().unwrap();
    let mut fields: Vec<&str> = props.keys().map(String::as_str).collect();
    fields.sort_unstable();
    assert_eq!(
        fields,
        ["colors", "fade_colors", "shape", "trail", "twinkle"],
        "the explosion object's fields"
    );
    for list in ["colors", "fade_colors"] {
        let text = props[list].to_string();
        assert!(
            text.contains(delvewright_dsl::color::HEX_PATTERN),
            "`{list}` carries the `#rrggbb` pattern: {text}"
        );
    }
    let shape = defs
        .get("FireworkShape")
        .expect("the shape is its own schema definition");
    let variants: Vec<&str> = shape["oneOf"]
        .as_array()
        .expect("the shape is a union of string constants")
        .iter()
        .filter_map(|v| v.get("const").and_then(Value::as_str))
        .collect();
    assert_eq!(
        variants,
        ["small_ball", "large_ball", "star", "creeper", "burst"],
        "exactly the game's five shapes, spelled as the component spells them"
    );
}

/// The effect union names thirty-eight verbs, and `firework` is one of them —
/// the enumeration a `DW0100` refusal prints back at an author who wrote an
/// unknown one.
#[test]
fn the_effect_union_names_thirty_eight_verbs() {
    let schema = stage_schema(Stage::Quests);
    let branches = verb_branches(&schema);
    let verbs: Vec<&str> = branches.iter().filter_map(verb_tag).collect();
    assert_eq!(
        verbs.len(),
        branches.len(),
        "every branch of the union is tagged with its verb"
    );
    assert!(
        verbs.contains(&"firework"),
        "the vocabulary carries `firework`: {verbs:?}"
    );
    assert_eq!(verbs.len(), 38, "the effect vocabulary's size: {verbs:?}");
}

// ---------------------------------------------------------------------------
// Criterion 7 — the pinned facts, in one file
// ---------------------------------------------------------------------------

/// Every game fact the firework rests on, and the pages it was read from. A
/// re-pin against a later Minecraft is one diff in `dsl::firework` and one here;
/// nothing else in the engine states any of these numbers.
#[test]
fn the_pinned_facts_are_what_the_pages_say() {
    assert_eq!(
        firework::WIKI_PAGES,
        [
            "Firework Rocket",
            "Data component format/fireworks",
            "Data component format/equippable",
        ],
        "the pages every constant below was read from"
    );

    // The five shapes, spelled as the component spells them.
    assert_eq!(firework::FireworkShape::ALL.len(), 5);
    assert_eq!(
        firework::FireworkShape::ALL.map(|s| s.token()),
        ["small_ball", "large_ball", "star", "creeper", "burst"]
    );

    // The `LifeTime` formula's fixed term, and the floor the emitter writes.
    assert_eq!(firework::LIFETIME_STEP_TICKS, 10);
    assert_eq!(firework::lifetime_ticks(1), 20);
    assert_eq!(firework::lifetime_ticks(2), 30);
    assert_eq!(firework::lifetime_ticks(3), 40);

    // The burst heights that follow from it.
    assert_eq!(firework::BURST_HEIGHTS, [8, 18, 32]);
    assert_eq!(firework::burst_height(1), 8);
    assert_eq!(firework::burst_height(2), 18);
    assert_eq!(firework::burst_height(3), 32);

    // The blast, and why seven stars is the cap.
    assert_eq!(firework::BLAST_RADIUS, 5);
    assert_eq!(firework::DAMAGE_ONE_STAR_HP, 7);
    assert_eq!(firework::DAMAGE_PER_EXTRA_STAR_HP, 2);
    assert_eq!(firework::MAX_EXPLOSIONS, 7);
    assert_eq!(firework::worst_damage_hp(), 19);
    assert!(
        firework::worst_damage_hp() < 20,
        "no rocket this verb writes can kill an unhurt player by itself"
    );

    // The entity, the component, and the entity's item field.
    assert_eq!(firework::ROCKET_ENTITY, "minecraft:firework_rocket");
    assert_eq!(firework::ROCKET_ITEM, "minecraft:firework_rocket");
    assert_eq!(firework::FIREWORKS_COMPONENT, "minecraft:fireworks");
    assert_eq!(firework::ITEM_FIELD, "FireworksItem");
}

/// `#ffd700` is `16766720` — criterion 3's first half, and the packing both the
/// firework star and the potion bottle read.
#[test]
fn a_colour_packs_to_what_vanilla_stores() {
    assert_eq!(delvewright_dsl::color::packed("#ffd700"), Some(16_766_720));
    assert_eq!(delvewright_dsl::color::packed("#ffffff"), Some(16_777_215));
    assert_eq!(delvewright_dsl::color::packed("#8b0000"), Some(9_109_504));
    assert_eq!(delvewright_dsl::color::packed("#ff9c30"), Some(16_751_664));
    assert_eq!(delvewright_dsl::color::packed("nope"), None);
}
