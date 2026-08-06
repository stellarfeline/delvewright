//! DSL v0.8 `collect` container adoption (task #95, owner ruling from island
//! playtest rounds 1 and 2).
//!
//! Three fields, one complaint: the quest item was an unnamed generic stack in a
//! chest the compiler conjured out of the air beside the barrel the prefab had
//! already put there, and opening it showed one lonely item. `container` adopts
//! the prefab's barrel, `item_name` gives the item a name a player can read, and
//! `fill_count` pads the container so it reads full.
//!
//! Same contract every reserved field before them kept: declaring any of them
//! below 0.8.0 is `DW0141`, and absence emits byte-identically to every campaign
//! written before them.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, l10n, parse_campaign};

/// hello-world's quests stage with one `collect` objective, parameterised on the
/// stage version and on the objective's own adoption fields.
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
          {{ "type": "collect", "id": "obj/cheese", "item": "minecraft:bread", "count": 3,
             "anchor": "anchor/exit", "after": ["obj/talk"]{fields} }}
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
    }
}

/// The whole surface at once: adopt the prefab's container, name the item, pad
/// the container to read full.
const ADOPTED: &str = r#", "container": "anchor/door", "item_name": "Cheese", "fill_count": 8"#;

#[test]
fn the_adoption_surface_validates_clean_at_v08() {
    let diags = check_campaign(&campaign_with(quests("0.8.0", ADOPTED)));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.8 adopted-container collect, got: {diags:#?}"
    );
}

/// Each field is independently reserved before 0.8.0 (`DW0141`): a 0.6/0.7
/// campaign's chest, its unnamed item and its single stack cannot move by a byte.
#[test]
fn each_adoption_field_is_reserved_before_v08() {
    for (fields, field) in [
        (r#", "container": "anchor/door""#, "container"),
        (r#", "item_name": "Cheese""#, "item_name"),
        (r#", "fill_count": 8"#, "fill_count"),
    ] {
        let diags = check_campaign(&campaign_with(quests("0.7.0", fields)));
        let d = diags
            .iter()
            .find(|d| d.code == "DW0141" && d.path.ends_with(field))
            .unwrap_or_else(|| panic!("expected DW0141 on `{field}`, got: {diags:#?}"));
        assert!(d.message.contains("0.8.0"), "{}", d.message);
    }
}

/// `fill_count: 0` is the default and means "no padding", so writing it out
/// explicitly is not a declaration of the reserved field — that would make the
/// serialized form of an ordinary pre-0.8 collect illegal to itself.
#[test]
fn an_explicit_zero_fill_count_is_not_a_reserved_declaration() {
    let diags = check_campaign(&campaign_with(quests("0.7.0", r#", "fill_count": 0"#)));
    assert!(
        !diags.iter().any(|d| d.code == "DW0141"),
        "a zero fill_count declares nothing: {diags:#?}"
    );
}

/// The container anchor is an anchor like any other: invented ones are `DW0142`.
#[test]
fn an_invented_container_anchor_is_dw0142() {
    let diags = check_campaign(&campaign_with(quests(
        "0.8.0",
        r#", "container": "anchor/no-such-barrel""#,
    )));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0142")
        .unwrap_or_else(|| panic!("expected DW0142, got: {diags:#?}"));
    assert!(d.path.ends_with("/container"), "{}", d.path);
}

/// The fill is positional, so it obeys the same 27-slot ceiling a `loot`
/// declaration does — and for the same reason: everything past the last slot is
/// dropped without a word.
#[test]
fn padding_past_the_last_slot_is_dw0432() {
    let diags = check_campaign(&campaign_with(quests(
        "0.8.0",
        r#", "container": "anchor/door", "fill_count": 27"#,
    )));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0432")
        .unwrap_or_else(|| panic!("expected DW0432, got: {diags:#?}"));
    assert!(d.path.ends_with("/fill_count"), "{}", d.path);
    assert!(d.message.contains("28 slots"), "{}", d.message);

    // The exact ceiling validates: 26 padding stacks + the objective's own = 27.
    assert!(
        !check_campaign(&campaign_with(quests(
            "0.8.0",
            r#", "container": "anchor/door", "fill_count": 26"#,
        )))
        .iter()
        .any(|d| d.code == "DW0432"),
        "a fill that exactly fills the container must validate"
    );
}

/// A `loot` entry and an adopted `collect` on one container overwrite each other
/// slot-for-slot — the `DW0435` rule, reached through the second door.
#[test]
fn a_loot_entry_and_an_adopted_collect_on_one_anchor_is_dw0435() {
    let quests = format!(
        r#"{{
  "dsl_version": "0.8.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "collect", "id": "obj/cheese", "item": "minecraft:bread", "count": 3,
             "anchor": "anchor/exit", "after": ["obj/talk"]{ADOPTED} }}
        ],
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "loot": [
      {{ "id": "loot/galley-stores", "anchor": "anchor/door",
         "items": [ {{ "item": "minecraft:cooked_cod", "count": 3 }} ] }}
    ]
  }}
}}"#
    );
    let diags = check_campaign(&campaign_with(quests));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0435")
        .unwrap_or_else(|| panic!("expected DW0435, got: {diags:#?}"));
    assert!(d.path.ends_with("/container"), "{}", d.path);
    assert!(
        d.message.contains("loot/galley-stores") && d.message.contains("obj/cheese"),
        "the message must name both claimants: {}",
        d.message
    );
}

/// Two collects filling one container is the same collision: whichever activates
/// second replaces the first objective's items with its own.
#[test]
fn two_adopted_collects_on_one_anchor_is_dw0435() {
    let quests = r#"{
  "dsl_version": "0.8.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "collect", "id": "obj/cheese", "item": "minecraft:bread", "count": 1,
            "anchor": "anchor/exit", "after": ["obj/talk"], "container": "anchor/door" },
          { "type": "collect", "id": "obj/wine", "item": "minecraft:potion", "count": 1,
            "anchor": "anchor/exit", "after": ["obj/cheese"], "container": "anchor/door" }
        ],
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;
    let diags = check_campaign(&campaign_with(quests.to_string()));
    assert!(
        diags.iter().any(|d| d.code == "DW0435"),
        "expected DW0435, got: {diags:#?}"
    );
}

/// The item's display name is read by a player off the stack in the barrel, so it
/// translates like every other player-visible string.
#[test]
fn item_name_enters_the_l10n_inventory() {
    let c = parse_campaign(&campaign_with(quests("0.8.0", ADOPTED))).expect("parses");
    let inv = l10n::inventory(&c);
    assert_eq!(
        inv.get("obj.open-the-door.cheese.item_name")
            .map(String::as_str),
        Some("Cheese"),
        "inventory keys were: {:?}",
        inv.keys().collect::<Vec<_>>()
    );
}

/// The stage-5 schema export carries all three fields (the skill authors against
/// it).
#[test]
fn stage5_schema_exports_the_adoption_fields() {
    let schema = delvewright_dsl::schema::stage_schema(delvewright_dsl::Stage::Quests);
    let json = serde_json::to_string(&schema).unwrap();
    for field in ["\"container\"", "\"item_name\"", "\"fill_count\""] {
        assert!(json.contains(field), "stage-5 schema is missing {field}");
    }
}

/// A collect that declares none of it serializes exactly as it did before the
/// fields existed — the byte-identity contract, checked where it is decidable
/// from the DSL alone.
#[test]
fn an_unadopted_collect_serializes_without_the_new_fields() {
    let c = parse_campaign(&campaign_with(quests("0.8.0", ""))).expect("parses");
    let json = serde_json::to_string(&c.quests.content).expect("serializes");
    for field in ["container", "item_name", "fill_count"] {
        assert!(
            !json.contains(field),
            "an unadopted collect must not serialize `{field}`: {json}"
        );
    }
}
