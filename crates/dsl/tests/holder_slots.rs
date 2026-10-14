//! spec-0111 §6.1: `DW1035`'s document arm — a fill's `slot` is in some
//! holder family's vocabulary, named once, on every stack or none.
//!
//! The family of the holder at the anchor needs the assembled world, so a
//! slot the holder actually standing there lacks is a build-tier proof
//! (`DW0431` / `DW0438`), and so are a stand fill with an unnamed stack and a
//! `collect` padding a frame or a stand (`DW1035` at the build tier). An
//! armour slot's `equippable` arm reads the pinned item data, which only the
//! compiler's registry carries; it is held in `crates/delvec/tests/holders.rs`.

mod common;

use delvewright_dsl::{DSL_VERSION, RawCampaign, check_campaign};

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
    "loot": [ {LOOT} ]
  }
}"#;

fn with_loot(loot: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: QUESTS_TMPL
            .replacen("{VER}", DSL_VERSION, 1)
            .replacen("{LOOT}", loot, 1),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

fn fill(items: &str) -> String {
    format!(r#"{{ "id": "loot/rack", "anchor": "anchor/exit", "items": [ {items} ] }}"#)
}

fn dw1035(items: &str) -> Vec<String> {
    check_campaign(&with_loot(&fill(items)))
        .into_iter()
        .filter(|d| d.code == "DW1035")
        .map(|d| d.message)
        .collect()
}

/// Every family's spelling passes the document tier: the holder at the anchor
/// is the build's question.
#[test]
fn a_slot_in_some_vocabulary_passes() {
    for items in [
        r#"{ "item": "minecraft:iron_sword", "slot": "weapon.mainhand" },
           { "item": "minecraft:leather_boots", "slot": "armor.head" }"#,
        r#"{ "item": "minecraft:torch", "slot": "contents" }"#,
        r#"{ "item": "minecraft:bread", "slot": "container.4" },
           { "item": "minecraft:cooked_beef", "slot": "container.0" }"#,
        r#"{ "item": "minecraft:bread" }, { "item": "minecraft:cooked_beef" }"#,
    ] {
        let diags = check_campaign(&with_loot(&fill(items)));
        assert!(diags.is_empty(), "{items}: {diags:#?}");
    }
}

#[test]
fn a_slot_no_holder_has_is_dw1035_printing_every_vocabulary() {
    let m = dw1035(r#"{ "item": "minecraft:iron_sword", "slot": "weapon.main" }"#);
    assert_eq!(m.len(), 1, "{m:#?}");
    assert!(m[0].contains("`weapon.main`"), "{}", m[0]);
    for word in [
        "`contents`",
        "`armor.head`",
        "`container.<n>`",
        "take only a stack",
    ] {
        assert!(m[0].contains(word), "{word}: {}", m[0]);
    }
    // Past the largest container's 27 slots is outside the vocabulary too.
    assert_eq!(
        dw1035(r#"{ "item": "minecraft:bread", "slot": "container.27" }"#).len(),
        1
    );
}

#[test]
fn a_slot_named_twice_is_dw1035() {
    let m = dw1035(
        r#"{ "item": "minecraft:iron_sword", "slot": "weapon.mainhand" },
           { "item": "minecraft:stone_sword", "slot": "weapon.mainhand" }"#,
    );
    assert_eq!(m.len(), 1, "{m:#?}");
    assert!(m[0].contains("twice"), "{}", m[0]);
}

#[test]
fn a_fill_mixing_named_and_unnamed_stacks_is_dw1035() {
    let m = dw1035(
        r#"{ "item": "minecraft:iron_sword", "slot": "weapon.mainhand" },
           { "item": "minecraft:bread" }"#,
    );
    assert_eq!(m.len(), 1, "{m:#?}");
    assert!(m[0].contains("1 of its 2 stacks"), "{}", m[0]);
}
