//! DSL v0.9 declared drops: an elite or
//! boss leaves behind a **declared subset** — usually one worn piece, plus any
//! quest token the fight yields — never automatically everything.
//!
//! Covered here: the version fence (`DW0141`), the worn-piece rule (`DW0490`),
//! the tier rule (`DW0491`), and the two halves of the drop-gated `collect`
//! proof — that the wave really yields the item (`DW0492`) and that the fight is
//! proven to happen first (`DW0493`).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.9 quests document shaped like the bell remake's gate boss: a boss wave
/// that wears an axe and a helm, drops **only** the axe, and yields a key the
/// door quest then collects.
const QUESTS_V09: &str = r#"{
  "dsl_version": "0.9.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "kill", "id": "obj/boss", "wave": "wave/gate-boss", "after": ["obj/talk"] },
          { "type": "collect", "id": "obj/key", "item": "minecraft:bread", "count": 1,
            "anchor": "anchor/keeper-stand", "dropped_by": "wave/gate-boss",
            "item_name": "Gate Key", "after": ["obj/boss"] },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/key"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "spawn-wave", "wave": "wave/gate-boss" } ],
          "obj/key": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "waves": [
      {
        "id": "wave/gate-boss",
        "anchor": "anchor/keeper-stand",
        "tier": "boss",
        "mobs": [
          { "entity": "minecraft:zombie", "count": 1,
            "equipment": { "feet": "minecraft:leather_boots", "main_hand": "minecraft:iron_sword" },
            "drops": [
              { "slot": "main_hand" },
              { "item": "minecraft:bread", "name": "Gate Key" }
            ] }
        ]
      }
    ]
  }
}"#;

fn campaign_with_quests(quests: &str) -> RawCampaign {
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
    }
}

fn codes(quests: &str) -> Vec<String> {
    check_campaign(&campaign_with_quests(quests))
        .into_iter()
        .map(|d| d.code)
        .collect()
}

/// The whole surface — a boss wave that drops one worn piece plus a quest token,
/// and the collect that takes the token off it — validates clean under 0.9.0.
#[test]
fn declared_drops_validate_clean() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V09));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.9 declared drop, got: {diags:#?}"
    );
}

/// `drops[]` (and `dropped_by`) under a pre-0.9 quests version are reserved.
#[test]
fn drops_reserved_before_0_9() {
    let pre = QUESTS_V09.replacen("\"0.9.0\"", "\"0.8.0\"", 1);
    assert!(
        codes(&pre).iter().any(|c| c == "DW0141"),
        "drops must be reserved under 0.8.0 (DW0141): {:#?}",
        codes(&pre)
    );
}

/// A `slot` drop naming a slot the entity's `equipment` does not fill is
/// `DW0490` — a body cannot leave behind a piece it never wore.
#[test]
fn slot_drop_must_be_worn() {
    let bad = QUESTS_V09.replacen("{ \"slot\": \"main_hand\" }", "{ \"slot\": \"legs\" }", 1);
    let got = codes(&bad);
    assert!(
        got.iter().any(|c| c == "DW0490"),
        "an unworn slot must be DW0490: {got:#?}"
    );
    // The message names BOTH sides: the slot asked for and what is actually worn.
    let d = check_campaign(&campaign_with_quests(&bad));
    let hit = d.iter().find(|d| d.code == "DW0490").expect("DW0490");
    assert!(hit.message.contains("legs"), "{}", hit.message);
    assert!(hit.message.contains("feet"), "{}", hit.message);
    assert!(hit.message.contains("main_hand"), "{}", hit.message);
}

/// The same slot declared twice is `DW0490` — a body leaves each piece once.
#[test]
fn duplicate_slot_drop_is_rejected() {
    let bad = QUESTS_V09.replacen(
        "{ \"slot\": \"main_hand\" },",
        "{ \"slot\": \"main_hand\" }, { \"slot\": \"main_hand\" },",
        1,
    );
    assert!(
        codes(&bad).iter().any(|c| c == "DW0490"),
        "a duplicate slot drop must be DW0490: {:#?}",
        codes(&bad)
    );
}

/// Drops on an untiered wave are `DW0491`: rank-and-file gear is never farmable.
#[test]
fn drops_require_an_elite_or_boss_tier() {
    let bad = QUESTS_V09.replacen("\"tier\": \"boss\",", "", 1);
    let got = codes(&bad);
    assert!(
        got.iter().any(|c| c == "DW0491"),
        "an untiered wave with drops must be DW0491: {got:#?}"
    );
}

/// An actor may declare drops too — same rules, same codes (`DW0491` when the
/// actor is not billed elite/boss).
#[test]
fn actor_drops_follow_the_same_tier_rule() {
    let with_actor = QUESTS_V09.replacen(
        "\"waves\": [",
        r#""actors": [
      { "id": "actor/warden", "entity": "minecraft:zombie", "anchor": "anchor/keeper-stand",
        "equipment": { "feet": "minecraft:leather_boots" },
        "drops": [ { "slot": "feet" } ] }
    ],
    "waves": ["#,
        1,
    );
    let got = codes(&with_actor);
    assert!(
        got.iter().any(|c| c == "DW0491"),
        "an untiered actor with drops must be DW0491: {got:#?}"
    );
    let tiered = with_actor.replacen(
        "\"anchor\": \"anchor/keeper-stand\",\n        \"equipment\"",
        "\"anchor\": \"anchor/keeper-stand\", \"tier\": \"elite\",\n        \"equipment\"",
        1,
    );
    let ok = codes(&tiered);
    assert!(
        !ok.iter().any(|c| c == "DW0491"),
        "an elite actor may declare drops: {ok:#?}"
    );
}

/// A `dropped_by` collect whose wave declares no such item is `DW0492`.
#[test]
fn drop_gated_collect_must_be_sourced() {
    let bad = QUESTS_V09.replacen(
        "{ \"item\": \"minecraft:bread\", \"name\": \"Gate Key\" }",
        "{ \"item\": \"minecraft:torch\", \"name\": \"Gate Key\" }",
        1,
    );
    let got = codes(&bad);
    assert!(
        got.iter().any(|c| c == "DW0492"),
        "an unsourced drop-gated collect must be DW0492: {got:#?}"
    );
}

/// A `dropped_by` collect that also adopts a `container` is `DW0492`: the item
/// comes off a body or out of a box, never both.
#[test]
fn drop_gated_collect_cannot_also_adopt_a_container() {
    let bad = QUESTS_V09.replacen(
        "\"dropped_by\": \"wave/gate-boss\",",
        "\"dropped_by\": \"wave/gate-boss\", \"container\": \"anchor/keeper-stand\",",
        1,
    );
    assert!(
        codes(&bad).iter().any(|c| c == "DW0492"),
        "container + dropped_by must be DW0492: {:#?}",
        codes(&bad)
    );
}

/// A `dropped_by` collect asking for more copies than the wave can yield is
/// `DW0492` — a body drops its declared item once.
#[test]
fn drop_gated_collect_cannot_outrun_the_wave() {
    let bad = QUESTS_V09.replacen(
        "\"item\": \"minecraft:bread\", \"count\": 1,",
        "\"item\": \"minecraft:bread\", \"count\": 3,",
        1,
    );
    assert!(
        codes(&bad).iter().any(|c| c == "DW0492"),
        "an over-asking drop-gated collect must be DW0492: {:#?}",
        codes(&bad)
    );
}

/// A `dropped_by` collect not ordered after the wave's `kill` is `DW0493` —
/// the "kill the boss → take its key → open the door" chain must be provable,
/// not merely intended.
#[test]
fn drop_gated_collect_must_follow_the_kill() {
    let bad = QUESTS_V09.replacen(
        "\"after\": [\"obj/boss\"] }",
        "\"after\": [\"obj/talk\"] }",
        1,
    );
    let got = codes(&bad);
    assert!(
        got.iter().any(|c| c == "DW0493"),
        "an unordered drop-gated collect must be DW0493: {got:#?}"
    );
}

/// `dropped_by` naming a wave that does not exist is the ordinary dangling
/// wave reference (`DW0170`), not a drop-specific code.
#[test]
fn drop_gated_collect_names_a_declared_wave() {
    let bad = QUESTS_V09.replacen(
        "\"dropped_by\": \"wave/gate-boss\"",
        "\"dropped_by\": \"wave/nobody\"",
        1,
    );
    assert!(
        codes(&bad).iter().any(|c| c == "DW0170"),
        "an unknown dropped_by wave must be DW0170: {:#?}",
        codes(&bad)
    );
}

/// A quest-item drop whose id is not in the pinned registry is `DW0143` — the
/// give-item family, exactly like every other item id in the DSL.
#[test]
fn quest_item_drop_id_is_registry_checked() {
    let bad = QUESTS_V09.replace("minecraft:bread", "minecraft:not_a_real_item");
    assert!(
        codes(&bad).iter().any(|c| c == "DW0143"),
        "an unknown drop item id must be DW0143: {:#?}",
        codes(&bad)
    );
}

/// A declared quest-item drop's display name is player-visible, so it enters the
/// authoritative l10n key inventory and translates like any other line.
#[test]
fn quest_item_drop_name_is_inventoried() {
    let c = delvewright_dsl::parse_campaign(&campaign_with_quests(QUESTS_V09)).expect("parses");
    let inv = delvewright_dsl::l10n_inventory(&c);
    assert!(
        inv.contains_key("wave.gate-boss.mob.0.drop.1.name"),
        "drop display names must be inventoried: {:#?}",
        inv.keys().collect::<Vec<_>>()
    );
}
