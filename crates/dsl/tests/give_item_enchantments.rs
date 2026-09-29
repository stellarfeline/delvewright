//! spec-0075: `give-item` describes the stack it hands over the way every other
//! item surface does, enchantments included — a shop can sell an enchanted
//! sword and a quest can hand over an enchanted book.
//!
//! The field is a `loot` stack's `enchantments`, under the same checks
//! (`DW0433` unknown id, `DW0434` level outside `1..=255`), at every effect root
//! and at any nesting depth. Before this surface existed the key was an unknown
//! field of the verb (`DW0100`).

mod common;

use delvewright_dsl::{
    DSL_VERSION, ItemRegistry, QuestEffect, RawCampaign, VendoredAnchorRegistry,
    VendoredEntityRegistry, VendoredItemRegistry, Verb, enchantment_component, parse_campaign,
    validate_campaign_with,
};

/// The crate's vendored item subset, plus the two items these fixtures hand
/// over. The compiler validates against the full pinned registry; this crate's
/// subset is its M1 fixtures', and an unknown item (`DW0143`) is not what these
/// tests are about.
struct Items(VendoredItemRegistry);

impl ItemRegistry for Items {
    fn contains(&self, id: &str) -> bool {
        matches!(id, "minecraft:iron_sword" | "minecraft:enchanted_book") || self.0.contains(id)
    }
}

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
        "on_objective_complete": { "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" }{TALK} ] },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]{EXTRA}
  }
}"#;

fn campaign(talk: &str, extra: &str) -> RawCampaign {
    let quests = QUESTS_TMPL
        .replacen("{VER}", DSL_VERSION, 1)
        .replacen("{TALK}", talk, 1)
        .replacen("{EXTRA}", extra, 1);
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests,
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

fn codes(c: &RawCampaign) -> Vec<(String, String)> {
    let campaign = match parse_campaign(c) {
        Ok(c) => c,
        Err(d) => return d.iter().map(|d| (d.code.clone(), d.path.clone())).collect(),
    };
    validate_campaign_with(
        &campaign,
        &Items(VendoredItemRegistry::v1_21_11()),
        &VendoredAnchorRegistry::hello_world(),
        &VendoredEntityRegistry::v1_21_11(),
    )
    .iter()
    .map(|d| (d.code.clone(), d.path.clone()))
    .collect()
}

const SWORD: &str = r#", { "type": "give-item", "item": "minecraft:iron_sword", "count": 1,
      "enchantments": { "minecraft:sharpness": 2 } }"#;
const BOOK: &str = r#", { "type": "give-item", "item": "minecraft:enchanted_book", "count": 1,
      "enchantments": { "minecraft:mending": 1 } }"#;

/// The red half: the key parses and validates clean on a quest bundle, for a
/// weapon and for a book.
#[test]
fn an_enchanted_give_validates_clean() {
    let c = campaign(&format!("{SWORD}{BOOK}"), "");
    let d = codes(&c);
    assert!(d.is_empty(), "{d:#?}");
}

/// The field round-trips into the verb, in id order (a `BTreeMap`).
#[test]
fn the_enchantments_reach_the_verb() {
    let parsed: QuestEffect = serde_json::from_str(
        r#"{ "type": "give-item", "item": "minecraft:iron_sword", "count": 1,
             "enchantments": { "minecraft:sharpness": 2, "minecraft:knockback": 1 } }"#,
    )
    .unwrap();
    let Verb::GiveItem { enchantments, .. } = &parsed.verb else {
        panic!("not a give-item: {parsed:?}")
    };
    assert_eq!(
        enchantments.iter().collect::<Vec<_>>(),
        vec![
            (&"minecraft:knockback".to_string(), &1),
            (&"minecraft:sharpness".to_string(), &2)
        ]
    );
    // …and an unenchanted give serialises exactly as before.
    let plain: QuestEffect =
        serde_json::from_str(r#"{ "type": "give-item", "item": "minecraft:bread", "count": 2 }"#)
            .unwrap();
    assert!(
        !serde_json::to_string(&plain)
            .unwrap()
            .contains("enchantments")
    );
}

/// `DW0433`/`DW0434` fire on a give exactly as on a loot stack, with the path
/// pointing into the verb's own `enchantments`.
#[test]
fn an_unknown_enchantment_and_a_zero_level_are_refused() {
    let bad = r#", { "type": "give-item", "item": "minecraft:iron_sword", "count": 1,
      "enchantments": { "minecraft:not_a_thing": 1, "minecraft:sharpness": 0 } }"#;
    let d = codes(&campaign(bad, ""));
    assert!(
        d.iter().any(|(c, p)| c == "DW0433"
            && p == "/content/quests/0/on_objective_complete/obj/talk/1/enchantments/minecraft:not_a_thing"),
        "{d:#?}"
    );
    assert!(
        d.iter()
            .any(|(c, p)| c == "DW0434" && p.ends_with("/1/enchantments/minecraft:sharpness")),
        "{d:#?}"
    );
}

/// The check reaches every root at any depth: a shop offer (the surface the
/// playtest asked for) and a `sequence` step inside one.
#[test]
fn the_check_reaches_a_shop_offer_and_a_nested_step() {
    let shop = r#",
    "shops": [
      { "id": "shop/smith", "anchor": "spawn", "title": "The smith",
        "offers": [
          { "label": "A book", "effects": [
              { "type": "give-item", "item": "minecraft:enchanted_book", "count": 1,
                "enchantments": { "minecraft:no_such_book": 1 } },
              { "type": "sequence", "steps": [ { "at_ticks": 10, "effects": [
                  { "type": "give-item", "item": "minecraft:iron_sword", "count": 1,
                    "enchantments": { "minecraft:sharpness": 300 } } ] } ] }
          ] }
        ] }
    ]"#;
    let d = codes(&campaign("", shop));
    assert!(
        d.iter()
            .any(|(c, p)| c == "DW0433" && p.contains("/content/shops/0/offers/0/effects/0/")),
        "{d:#?}"
    );
    assert!(
        d.iter()
            .any(|(c, p)| c == "DW0434" && p.contains("/steps/0/effects/0/enchantments/")),
        "{d:#?}"
    );
}

/// The component rule is vanilla's: a book stores, everything else carries.
#[test]
fn a_book_stores_its_enchantments() {
    assert_eq!(
        enchantment_component("minecraft:enchanted_book"),
        "minecraft:stored_enchantments"
    );
    assert_eq!(
        enchantment_component("minecraft:iron_sword"),
        "minecraft:enchantments"
    );
    assert_eq!(
        enchantment_component("minecraft:book"),
        "minecraft:enchantments",
        "a plain book is not an enchanted book"
    );
}
