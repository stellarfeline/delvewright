//! DSL v0.81 `collect.gathered`: the world supplies the item, and the engine
//! places nothing (`DW1121` for a field that would place it).

use crate::common;

use delvewright_dsl::{DSL_VERSION, RawCampaign, check_campaign};

/// hello-world's quests stage with one gathered `collect`, plus `fields`.
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
          {{ "type": "collect", "id": "obj/fish", "item": "minecraft:bread", "count": 5,
             "anchor": "anchor/exit", "gathered": true, "after": ["obj/talk"]{fields} }}
        ],
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn campaign_with(quests: String) -> RawCampaign {
    common::declared(RawCampaign {
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
    })
}

#[test]
fn a_gathered_collect_validates_clean() {
    let diags = check_campaign(&campaign_with(quests("")));
    assert!(diags.is_empty(), "{diags:#?}");
}

/// Each field that places the item is refused beside `gathered`, at its own
/// pointer, naming the field; one at a time, so the count is the field's.
#[test]
fn a_field_that_places_the_item_beside_gathered_is_dw1121() {
    for (field, json) in [
        ("holder", r#", "holder": "anchor/door""#),
        ("item_name", r#", "item_name": "Supper""#),
        ("fill_count", r#", "fill_count": 2"#),
        ("dropped_by", r#", "dropped_by": "wave/none""#),
    ] {
        let diags = check_campaign(&campaign_with(quests(json)));
        let hits: Vec<_> = diags.iter().filter(|d| d.code == "DW1121").collect();
        assert_eq!(hits.len(), 1, "{field}: {diags:#?}");
        assert!(
            hits[0].path.ends_with(&format!("/{field}")),
            "{field}: {}",
            hits[0].path
        );
        assert!(
            hits[0].message.contains(&format!("and `{field}`")),
            "{field}: {}",
            hits[0].message
        );
    }
}

/// `fill_count: 0` is the default, and declares no padding.
#[test]
fn a_zero_fill_count_is_no_placement() {
    let diags = check_campaign(&campaign_with(quests(r#", "fill_count": 0"#)));
    assert!(!diags.iter().any(|d| d.code == "DW1121"), "{diags:#?}");
}

/// A `slot` beside `gathered` is `DW1035`'s, and says the item is placed nowhere.
#[test]
fn a_slot_beside_gathered_is_dw1035() {
    let diags = check_campaign(&campaign_with(quests(r#", "slot": "container.0""#)));
    let d = diags
        .iter()
        .find(|d| d.code == "DW1035")
        .unwrap_or_else(|| panic!("{diags:#?}"));
    assert!(d.message.contains("`gathered`"), "{}", d.message);
}
