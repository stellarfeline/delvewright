//! `DW0849`: **an item gate a class cannot bring.**
//!
//! ## The finding
//!
//! A required item was issued through one class's kit rather than to the party,
//! so a player who picked any other class arrived at the objective that consumed
//! it and could do nothing. The instance was repaired by moving the item. The
//! class of defect — *completability that depends on which class was picked* —
//! had no check at all, so the next item gate was free to reintroduce it.
//!
//! ## What is asserted here
//!
//! The rule quantifies over EVERY class, for the same reason `DW0476` does: a
//! delve is played by one to four players who each pick one class, so a solo
//! player of any class is a party the campaign must be finishable by.
//!
//! Falsifiability runs in both directions and both are exercised: the item
//! confined to one kit of two is refused; the same document with any ONE
//! class-blind supply added is clean. The three supplies are asserted
//! separately, because "some source discharges it" and "this source discharges
//! it" are different claims and only the second says the enumeration is right.
//!
//! The red demo is confirmed **not inert**: `DW0849` is `EveryVersion`, so
//! `dw0849_is_unfenced_so_the_red_is_not_a_version_artifact` drives the same red
//! document at the lowest and the highest supported quests version and requires
//! the diagnostic at both. A check keyed off a surface the fixture's version
//! never reached would pass that test only by accident, and this one cannot.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// Two classes, only one of which carries `stripped_oak_log`.
const TWO_CLASSES_ONE_CARRIER: &str = r#"{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "classes",
  "content": {
    "classes": [
      {
        "id": "class/wanderer",
        "name": "Wanderer",
        "blurb": "Sturdy boots, no questions.",
        "kit": [
          { "item": "minecraft:iron_sword", "count": 1 },
          { "item": "minecraft:stripped_oak_log", "count": 1 }
        ]
      },
      {
        "id": "class/warder",
        "name": "Warder",
        "blurb": "A shield and a long memory.",
        "kit": [ { "item": "minecraft:shield", "count": 1 } ]
      }
    ]
  }
}"#;

/// The same two classes, both carrying the gated item.
const TWO_CLASSES_BOTH_CARRY: &str = r#"{
  "dsl_version": "0.2.0",
  "campaign_id": "hello-world",
  "stage": "classes",
  "content": {
    "classes": [
      {
        "id": "class/wanderer",
        "name": "Wanderer",
        "blurb": "Sturdy boots, no questions.",
        "kit": [
          { "item": "minecraft:iron_sword", "count": 1 },
          { "item": "minecraft:stripped_oak_log", "count": 1 }
        ]
      },
      {
        "id": "class/warder",
        "name": "Warder",
        "blurb": "A shield and a long memory.",
        "kit": [
          { "item": "minecraft:shield", "count": 1 },
          { "item": "minecraft:stripped_oak_log", "count": 1 }
        ]
      }
    ]
  }
}"#;

/// hello-world's quest with an `interact` gated on `stripped_oak_log`, plus
/// whatever extra objective / effect / stage-5 section a case wants to supply
/// the item with.
fn quests(version: &str, extra_objective: &str, extra_effect: &str, extra_section: &str) -> String {
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
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }}{extra_objective},
          {{ "type": "interact", "id": "obj/pry", "anchor": "anchor/exit",
             "after": ["obj/talk"], "requires_item": "minecraft:stripped_oak_log" }}
        ],
        "on_complete": [ {{ "type": "campaign-complete" }}{extra_effect} ]
      }}
    ]{extra_section}
  }}
}}"#
    )
}

fn campaign(classes: &str, quests: String) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: classes.to_string(),
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

fn dw0849(diags: &[delvewright_dsl::Diagnostic]) -> Vec<&delvewright_dsl::Diagnostic> {
    diags.iter().filter(|d| d.code == "DW0849").collect()
}

/// **The finding, as a rule.** The item lives in one kit of two and nothing
/// class-blind supplies it, so a `class/warder` party can never press the thing.
#[test]
fn an_item_only_one_class_carries_is_refused() {
    let diags = check_campaign(&campaign(
        TWO_CLASSES_ONE_CARRIER,
        quests("0.7.0", "", "", ""),
    ));
    let hits = dw0849(&diags);
    assert_eq!(
        hits.len(),
        1,
        "expected exactly one DW0849 for an item only one class carries, got: {diags:#?}"
    );
    let m = &hits[0].message;
    assert!(
        m.contains("class/warder"),
        "DW0849 names the class that cannot bring it: {m}"
    );
    assert!(
        !m.contains("class/wanderer"),
        "DW0849 must NOT name the class that CAN bring it: {m}"
    );
    assert!(
        m.contains("minecraft:stripped_oak_log"),
        "DW0849 names the item: {m}"
    );
    assert!(m.contains("obj/pry"), "DW0849 names the objective: {m}");
    assert_eq!(
        hits[0].path, "/content/quests/0/objectives/1/requires_item",
        "DW0849 points at the gate, not at the class list"
    );
}

/// The stronger shape the same rule catches, which nobody has reported: the item
/// has no source in the campaign at all — a typo in an item id that every other
/// check accepts because the id is a real vanilla item.
#[test]
fn an_item_nothing_supplies_at_all_is_refused_and_says_so() {
    let no_carrier =
        TWO_CLASSES_ONE_CARRIER.replace("minecraft:stripped_oak_log", "minecraft:stick");
    let diags = check_campaign(&campaign(&no_carrier, quests("0.7.0", "", "", "")));
    let hits = dw0849(&diags);
    assert_eq!(
        hits.len(),
        1,
        "an item with no source anywhere is one DW0849, got: {diags:#?}"
    );
    assert!(
        hits[0]
            .message
            .contains("nothing in this campaign supplies it at all"),
        "the two shapes read differently — no supply is not 'another class's kit': {}",
        hits[0].message
    );
}

/// Green: every class carries it, so every party can finish.
#[test]
fn an_item_every_class_carries_is_clean() {
    let diags = check_campaign(&campaign(
        TWO_CLASSES_BOTH_CARRY,
        quests("0.7.0", "", "", ""),
    ));
    assert!(
        dw0849(&diags).is_empty(),
        "an item in every kit binds no class: {diags:#?}"
    );
}

/// Green via a `collect` — the item sits in a container anyone can open.
#[test]
fn a_collect_objective_discharges_the_gate() {
    let collect = r#",
          { "type": "collect", "id": "obj/gather", "item": "minecraft:stripped_oak_log",
            "count": 1, "anchor": "anchor/exit", "after": ["obj/talk"] }"#;
    let diags = check_campaign(&campaign(
        TWO_CLASSES_ONE_CARRIER,
        quests("0.7.0", collect, "", ""),
    ));
    assert!(
        dw0849(&diags).is_empty(),
        "a `collect` supplies the item to whoever opens the container: {diags:#?}"
    );
}

/// Green via a `give-item` — the default `carrier` is `all`, so the party has it.
#[test]
fn a_give_item_effect_discharges_the_gate() {
    let give = r#",
          { "type": "give-item", "item": "minecraft:stripped_oak_log", "count": 1 }"#;
    let diags = check_campaign(&campaign(
        TWO_CLASSES_ONE_CARRIER,
        quests("0.7.0", "", give, ""),
    ));
    assert!(
        dw0849(&diags).is_empty(),
        "a `give-item` supplies the item class-blind: {diags:#?}"
    );
}

/// Green via a stage-5 `loot` container — the third arm of the enumeration, and
/// the one a walk written from the two verbs an author remembers would miss.
#[test]
fn a_loot_container_discharges_the_gate() {
    let loot = r#",
    "loot": [
      { "id": "loot/woodpile", "anchor": "anchor/exit",
        "items": [ { "item": "minecraft:stripped_oak_log", "count": 1 } ] }
    ]"#;
    let diags = check_campaign(&campaign(
        TWO_CLASSES_ONE_CARRIER,
        quests("0.7.0", "", "", loot),
    ));
    assert!(
        dw0849(&diags).is_empty(),
        "a `loot` container supplies the item class-blind: {diags:#?}"
    );
}

/// **The red demo is not inert.** `DW0849` judges a contradiction between two
/// authored documents, so it is `EveryVersion` and no per-stage fence can
/// grandfather it away. Driving the identical red document at the bottom and the
/// top of the supported range and requiring the diagnostic at both is what says
/// so — the `unfenced` vacuity mode reds here rather than in a staging round.
#[test]
fn dw0849_is_unfenced_so_the_red_is_not_a_version_artifact() {
    for version in ["0.3.0", "0.14.0"] {
        let diags = check_campaign(&campaign(
            TWO_CLASSES_ONE_CARRIER,
            quests(version, "", "", ""),
        ));
        assert_eq!(
            dw0849(&diags).len(),
            1,
            "DW0849 must bind at quests {version}; a version-shaped hole here is the \
             `unfenced` vacuity mode: {diags:#?}"
        );
    }
}

/// The binding, stated. A campaign with no item gate is not silently green for a
/// reason nobody can see — it has nothing of the class to judge, and this test is
/// what makes that distinguishable from a check that stopped working.
#[test]
fn a_campaign_with_no_item_gate_binds_zero_and_says_nothing() {
    let ungated = quests("0.7.0", "", "", "")
        .replace(r#", "requires_item": "minecraft:stripped_oak_log""#, "");
    assert!(
        !ungated.contains("requires_item"),
        "the perturbation must actually remove the gate — a replacement that matched \
         nothing is a silent no-op"
    );
    let diags = check_campaign(&campaign(TWO_CLASSES_ONE_CARRIER, ungated));
    assert!(
        dw0849(&diags).is_empty(),
        "no item gate, nothing to say: {diags:#?}"
    );
}
