//! `DW0957`: an `interact` objective's prop is a block that invites a step.
//!
//! An `interact` completes on a right-click at its anchor. A pressure plate or a
//! tripwire tells the player to walk onto it, and walking onto it does nothing,
//! so a prop that fires on a step is refused where it is written. Which blocks
//! fire on a step is read from the pinned block registry
//! ([`delvewright_dsl::stepped_blocks`]); `crates/delvec/tests/stepped_blocks_tag.rs`
//! cross-checks that set against vanilla's own `#pressure_plates` tag.

mod common;

use delvewright_dsl::{DSL_VERSION, RawCampaign, check_campaign};

fn quests(prop: &str) -> String {
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
          {{ "type": "interact", "id": "obj/press", "anchor": "anchor/exit", "after": ["obj/talk"], "prop": {{ "block": "{prop}" }} }}
        ],
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

fn diags_for(prop: &str) -> Vec<delvewright_dsl::Diagnostic> {
    check_campaign(&RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests(prop),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
}

/// Every pressure plate and the tripwire string, with and without a blockstate,
/// is refused at the prop's own pointer.
#[test]
fn a_stepped_prop_is_dw0957() {
    for prop in [
        "minecraft:stone_pressure_plate",
        "minecraft:oak_pressure_plate",
        "minecraft:heavy_weighted_pressure_plate",
        "minecraft:polished_blackstone_pressure_plate[powered=false]",
        "minecraft:tripwire",
    ] {
        let d = diags_for(prop);
        let hit = d
            .iter()
            .find(|x| x.code == "DW0957")
            .unwrap_or_else(|| panic!("`{prop}` must be DW0957: {d:#?}"));
        assert_eq!(
            hit.path, "/content/quests/0/objectives/1/prop/block",
            "{hit:#?}"
        );
        assert!(
            hit.message.contains("on: step"),
            "the refusal names the step trigger: {}",
            hit.message
        );
    }
}

/// A block a hand works — a lever, a button — and the tripwire's HOOK, which a
/// step does not fire, are not refused.
#[test]
fn a_clicked_prop_is_not_dw0957() {
    for prop in [
        "minecraft:lever",
        "minecraft:stone_button",
        "minecraft:tripwire_hook",
    ] {
        let d = diags_for(prop);
        assert!(
            !d.iter().any(|x| x.code == "DW0957"),
            "`{prop}` is clicked, not stepped: {d:#?}"
        );
    }
}

/// The registry reading binds: sixteen plates and the string, read from the
/// pinned registry rather than listed here.
#[test]
fn the_stepped_set_is_read_from_the_pinned_registry() {
    let set = delvewright_dsl::stepped_blocks();
    assert_eq!(set.len(), 17, "{set:#?}");
    assert!(set.contains(&"minecraft:tripwire"));
    assert!(!set.contains(&"minecraft:tripwire_hook"));
    assert_eq!(
        set.iter().filter(|b| b.ends_with("_pressure_plate")).count(),
        16
    );
}
