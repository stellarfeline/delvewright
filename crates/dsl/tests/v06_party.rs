//! DSL v0.6 party surface (spec-0018): `world.min_players` and the `carrier`
//! field on `give-item` / class-kit items.
//!
//! Both are additive v0.6 fields, so they are reserved (`DW0141`) under an
//! earlier `dsl_version` on the stage that carries them, and absent by default —
//! every pre-0.6 campaign reads as a party of one whose items go to everyone.
//! `min_players` outside `1..=4` is `DW0356`; a `carrier: "one"` in a bundle only
//! the scheduler ever runs is `DW0357`.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

fn raw_with(world: Option<&str>, classes: Option<&str>, quests: Option<&str>) -> RawCampaign {
    RawCampaign {
        world: world
            .map(str::to_string)
            .unwrap_or_else(|| common::read_valid("world.json")),
        npcs: common::read_valid("npcs.json"),
        classes: classes
            .map(str::to_string)
            .unwrap_or_else(|| common::read_valid("classes.json")),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests
            .map(str::to_string)
            .unwrap_or_else(|| common::read_valid("quests.json")),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
}

/// hello-world's world stage at 0.6.0 with a declared `min_players`.
fn world_with_min_players(n: &str, version: &str) -> String {
    common::read_valid("world.json")
        .replacen("\"0.2.0\"", &format!("\"{version}\""), 1)
        .replacen(
            "\"target_minutes\": 5,",
            &format!("\"target_minutes\": 5,\n    \"min_players\": {n},"),
            1,
        )
}

/// A v0.6 quests document whose `give-item` carries `carrier`, at `position`
/// (`beat` = directly on a quest beat, `sequence` = inside a timeline step,
/// `arrive` = inside a `move-npc` `on_arrive`).
fn quests_with_carrier(position: &str) -> String {
    let give =
        r#"{ "type": "give-item", "item": "minecraft:torch", "count": 1, "carrier": "one" }"#;
    let effects = match position {
        "beat" => give.to_string(),
        "sequence" => format!(
            r#"{{ "type": "sequence", "steps": [ {{ "at_ticks": 0, "effects": [ {give} ] }} ] }}"#
        ),
        "arrive" => format!(
            r#"{{ "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
                   "on_arrive": [ {give} ] }}"#
        ),
        "respawn" => format!(
            r#"{{ "type": "set-checkpoint", "anchor": "anchor/exit",
                   "on_respawn": [ {give} ] }}"#
        ),
        other => panic!("unknown position `{other}`"),
    };
    format!(
        r#"{{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {{
    "quests": [
      {{
        "id": "quest/open-the-door",
        "trigger": {{ "type": "campaign-start" }},
        "objectives": [
          {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }}, {effects} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

// ---------------------------------------------------------------------------
// min_players
// ---------------------------------------------------------------------------

/// A declared party size inside `1..=4` validates clean under 0.6.0.
#[test]
fn min_players_in_range_validates_clean() {
    for n in ["1", "2", "3", "4"] {
        let diags = check_campaign(&raw_with(
            Some(&world_with_min_players(n, "0.6.0")),
            None,
            None,
        ));
        assert!(
            diags.is_empty(),
            "min_players {n} must validate clean: {diags:#?}"
        );
    }
}

/// A delve is played by ONE party of 1–4, so 0 and 5 are `DW0356`.
#[test]
fn min_players_out_of_range_is_dw0356() {
    for n in ["0", "5", "40"] {
        let diags = check_campaign(&raw_with(
            Some(&world_with_min_players(n, "0.6.0")),
            None,
            None,
        ));
        assert!(
            diags.iter().any(|d| d.code == "DW0356"),
            "min_players {n} must be DW0356: {diags:#?}"
        );
    }
}

/// `min_players` is a v0.6 field: reserved (`DW0141`) on an earlier world stage.
#[test]
fn min_players_reserved_before_0_6() {
    let diags = check_campaign(&raw_with(
        Some(&world_with_min_players("2", "0.5.0")),
        None,
        None,
    ));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "min_players must be reserved under 0.5.0: {diags:#?}"
    );
}

// ---------------------------------------------------------------------------
// carrier
// ---------------------------------------------------------------------------

/// `carrier: "one"` on a quest beat (a bundle a player completes) is legal.
#[test]
fn carrier_one_on_a_quest_beat_validates_clean() {
    let diags = check_campaign(&raw_with(None, None, Some(&quests_with_carrier("beat"))));
    assert!(
        diags.is_empty(),
        "carrier: one on a completed beat must validate clean: {diags:#?}"
    );
}

/// A scheduler-only bundle has no acting player, so `carrier: "one"` there has no
/// recipient: `DW0357`, for both scheduler seams.
#[test]
fn carrier_one_in_a_scheduled_bundle_is_dw0357() {
    for position in ["sequence", "arrive"] {
        let diags = check_campaign(&raw_with(None, None, Some(&quests_with_carrier(position))));
        assert!(
            diags.iter().any(|d| d.code == "DW0357"),
            "carrier: one inside a {position} bundle must be DW0357: {diags:#?}"
        );
    }
}

/// An `on_respawn` bundle IS dispatched per player, so `carrier: "one"` there
/// keeps its recipient — the walk must not latch through it.
#[test]
fn carrier_one_in_an_on_respawn_bundle_is_allowed() {
    let diags = check_campaign(&raw_with(None, None, Some(&quests_with_carrier("respawn"))));
    assert!(
        !diags.iter().any(|d| d.code == "DW0357"),
        "an on_respawn bundle has an acting player: {diags:#?}"
    );
}

/// `give-item.carrier` is a v0.6 field on the v0.3 verb: reserved earlier.
#[test]
fn give_item_carrier_reserved_before_0_6() {
    let pre = quests_with_carrier("beat").replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&raw_with(None, None, Some(&pre)));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "give-item carrier must be reserved under 0.5.0: {diags:#?}"
    );
}

/// A class kit may mark ONE item party-unique; the field is v0.6-gated on the
/// classes stage.
#[test]
fn kit_carrier_is_v06_gated() {
    let with_carrier = |version: &str| {
        common::patch_doc(&common::read_valid("classes.json"), |d| {
            d["dsl_version"] = serde_json::json!(version);
            let kit = d["content"]["classes"][0]["kit"]
                .as_array_mut()
                .expect("the wanderer has a kit");
            let bread = kit
                .iter_mut()
                .find(|i| i["item"] == "minecraft:bread")
                .expect("the wanderer's kit still carries bread");
            bread["carrier"] = serde_json::json!("one");
        })
    };
    let ok = check_campaign(&raw_with(None, Some(&with_carrier("0.6.0")), None));
    assert!(ok.is_empty(), "kit carrier is legal at 0.6.0: {ok:#?}");
    let pre = check_campaign(&raw_with(None, Some(&with_carrier("0.5.0")), None));
    assert!(
        pre.iter().any(|d| d.code == "DW0141"),
        "kit carrier must be reserved under 0.5.0: {pre:#?}"
    );
}
