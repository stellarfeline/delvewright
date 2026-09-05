//! DSL v0.7: `interact.requires_item` is a HELD gate, and `missing_item_hint` is
//! the diegetic answer to a click that arrives without the item in hand.
//! Version gating, the `DW0437` pairing rule, l10n and the schema export.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, l10n, parse_campaign};

/// hello-world's single quest with an `interact` objective, parameterised on the
/// stage version and on the objective's own item/hint fields so one document
/// serves every case below.
fn quests(version: &str, fields: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "interact", "id": "obj/pry", "anchor": "anchor/exit", "after": ["obj/talk"]{fields} }}
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
    }
}

const GATED: &str = r#", "requires_item": "minecraft:iron_sword", "missing_item_hint": "The bar will not shift with bare hands.""#;

/// A gated `interact` carrying its empty-hand line validates clean under v0.7.
#[test]
fn missing_item_hint_validates_clean_under_v07() {
    let diags = check_campaign(&campaign_with(quests("0.19.0", GATED)));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.7 held-item interact, got: {diags:#?}"
    );
}

/// `missing_item_hint` without `requires_item` is `DW0437`: the line answers a
/// gate that does not exist, so it could never narrate.
#[test]
fn missing_item_hint_without_requires_item_is_an_error() {
    let orphan = r#", "missing_item_hint": "The bar will not shift with bare hands.""#;
    let diags = check_campaign(&campaign_with(quests("0.19.0", orphan)));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0437")
        .unwrap_or_else(|| panic!("expected DW0437, got: {diags:#?}"));
    assert!(d.path.ends_with("/missing_item_hint"), "{}", d.path);
    assert!(
        d.message.contains("requires_item"),
        "the prescription must name the field to add: {}",
        d.message
    );

    // Control: the same hint WITH a gate is clean, so DW0437 is about the pairing
    // and not about the field's mere presence.
    assert!(
        !check_campaign(&campaign_with(quests("0.19.0", GATED)))
            .iter()
            .any(|d| d.code == "DW0437"),
        "a paired hint must not raise DW0437"
    );
}

/// The empty-hand line is spoken to a player, so it is translated like every other
/// player-visible string.
#[test]
fn missing_item_hint_enters_the_l10n_inventory() {
    let c = parse_campaign(&campaign_with(quests("0.19.0", GATED))).expect("parses");
    let inv = l10n::inventory(&c);
    let key = "obj.open-the-door.pry.missing_item_hint";
    assert_eq!(
        inv.get(key).map(String::as_str),
        Some("The bar will not shift with bare hands."),
        "inventory keys were: {:?}",
        inv.keys().collect::<Vec<_>>()
    );
}

/// The stage-5 schema export carries the field (the skill authors against it).
#[test]
fn stage5_schema_exports_missing_item_hint() {
    let schema = delvewright_dsl::schema::stage_schema(delvewright_dsl::Stage::Quests);
    let json = serde_json::to_string(&schema).unwrap();
    assert!(
        json.contains("\"missing_item_hint\""),
        "stage-5 schema must carry `missing_item_hint`"
    );
}
