//! spec-0114 §6.3: `interact.takes_item` — the presented item leaves the hand.
//! The `DW1046` pairing rule, the byte-identical round trip of a document that
//! omits it, and the schema export.

mod common;

use delvewright_dsl::{DSL_VERSION, RawCampaign, check_campaign, parse_campaign};

fn quests(fields: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "interact", "id": "obj/lay", "anchor": "anchor/exit", "after": ["obj/talk"]{fields} }}
        ],
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn campaign_with(quests: String) -> RawCampaign {
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

const TAKES: &str = r#", "requires_item": "minecraft:iron_sword", "takes_item": true"#;

/// A take of the presented item validates clean.
#[test]
fn a_take_of_the_presented_item_is_clean() {
    let diags = check_campaign(&campaign_with(quests(TAKES)));
    assert!(diags.is_empty(), "{diags:#?}");
}

/// `takes_item: true` with no `requires_item` is `DW1046`: nothing is presented,
/// so nothing can be taken.
#[test]
fn takes_item_without_requires_item_is_dw1046() {
    let diags = check_campaign(&campaign_with(quests(r#", "takes_item": true"#)));
    let d = diags
        .iter()
        .find(|d| d.code == "DW1046")
        .unwrap_or_else(|| panic!("expected DW1046, got: {diags:#?}"));
    assert_eq!(d.path, "/content/quests/0/objectives/1/takes_item");
    assert!(d.message.contains("requires_item"), "{}", d.message);
    // `false` is the default and binds nothing to refuse.
    assert!(
        !check_campaign(&campaign_with(quests(r#", "takes_item": false"#)))
            .iter()
            .any(|d| d.code == "DW1046")
    );
}

/// A document without the field canonicalises byte-identically, and `false`
/// is never written.
#[test]
fn an_absent_take_round_trips_byte_identically() {
    for fields in ["", r#", "requires_item": "minecraft:paper""#] {
        let c = parse_campaign(&campaign_with(quests(fields))).expect("parses");
        let doc = serde_json::to_value(&c.quests).unwrap();
        let o = &doc["content"]["quests"][0]["objectives"][1];
        assert!(o.get("takes_item").is_none(), "{o}");
        let again: delvewright_dsl::Envelope<delvewright_dsl::QuestsContent> =
            serde_json::from_value(doc.clone()).unwrap();
        assert_eq!(serde_json::to_value(&again).unwrap(), doc);
    }
    let c = parse_campaign(&campaign_with(quests(TAKES))).expect("parses");
    let doc = serde_json::to_value(&c.quests).unwrap();
    assert_eq!(
        doc["content"]["quests"][0]["objectives"][1]["takes_item"],
        true
    );
}

/// The schema exports `takes_item` on `interact` and on no other objective kind.
#[test]
fn the_schema_exports_takes_item_on_interact_only() {
    let schema = delvewright_dsl::schema::stage_schema(delvewright_dsl::Stage::Quests);
    let text = serde_json::to_string(&schema).unwrap();
    assert_eq!(
        text.matches("\"takes_item\"").count(),
        1,
        "exactly one site"
    );
    let variants = schema["$defs"]["Objective"]["oneOf"]
        .as_array()
        .expect("variants");
    for v in variants {
        let kind = v["properties"]["type"]["const"]
            .as_str()
            .unwrap_or_default();
        let has = v["properties"].get("takes_item").is_some();
        assert_eq!(has, kind == "interact", "{kind}");
    }
}
