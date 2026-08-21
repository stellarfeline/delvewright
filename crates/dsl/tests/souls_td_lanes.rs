//! spec-0016 §6 — the wave `lane` / `summon` surface.
//!
//! Every rule here is a live-verified 1.21.11 failure mode turned loud
//! (`docs/notes/td-routing-spike.md`). The DSL layer owns the five that are
//! decidable from the declaration alone; lane *geometry* (standable, walkable,
//! spaced > 10) and ring *occupancy* are build-tier proofs over the assembled
//! world (`DW0386`/`DW0387`, `crates/compiler/tests/souls_td_lanes.rs`).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// hello-world's quest stage at 0.6.0 with a raider lane and an aggro-edge wave.
const QUESTS_V06: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "kill", "id": "obj/warband", "wave": "wave/warband", "after": ["obj/talk"] },
          { "type": "kill", "id": "obj/spirits", "wave": "wave/spirits", "after": ["obj/talk"] },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/warband", "obj/spirits"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "spawn-wave", "wave": "wave/warband" },
            { "type": "spawn-wave", "wave": "wave/spirits" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "waves": [
      {
        "id": "wave/warband",
        "anchor": "anchor/keeper-stand",
        "mobs": [
          { "entity": "minecraft:pillager", "count": 2 },
          { "entity": "minecraft:vindicator", "count": 1 }
        ],
        "lane": { "waypoints": ["anchor/door", "anchor/exit"], "aggro_radius": 16 }
      },
      {
        "id": "wave/spirits",
        "anchor": "anchor/exit",
        "summon": "aggro-edge",
        "mobs": [
          { "entity": "minecraft:drowned", "count": 2, "attributes": { "follow_range": 12 } }
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
        site_plan: None,
        detail_plan: None,
    }
}

fn diags_for(quests: &str) -> Vec<delvewright_dsl::Diagnostic> {
    check_campaign(&campaign_with_quests(quests))
}

/// A well-formed lane and a well-formed aggro-edge wave validate clean.
#[test]
fn lane_and_aggro_edge_validate_clean() {
    let diags = diags_for(QUESTS_V06);
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.6 §6 surface, got: {diags:#?}"
    );
}

/// Both fields are v0.6 stage-5 surface: reserved under an earlier version, so a
/// pre-0.6 campaign that declares one is rejected rather than silently ignored.
#[test]
fn lane_and_summon_are_reserved_before_0_6() {
    let pre = QUESTS_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = diags_for(&pre);
    for path in ["/content/waves/0/lane", "/content/waves/1/summon"] {
        assert!(
            diags.iter().any(|d| d.code == "DW0141" && d.path == path),
            "{path} must be reserved under 0.5.0 (DW0141): {diags:#?}"
        );
    }
}

// --- DW0381: the declaration does not resolve / contradicts itself ---------

#[test]
fn an_empty_waypoint_list_is_dw0381() {
    let bad = QUESTS_V06.replace(
        "\"waypoints\": [\"anchor/door\", \"anchor/exit\"]",
        "\"waypoints\": []",
    );
    assert!(
        diags_for(&bad)
            .iter()
            .any(|d| d.code == "DW0381" && d.path == "/content/waves/0/lane/waypoints"),
        "a lane with no waypoints is DW0381"
    );
}

#[test]
fn an_invented_waypoint_anchor_is_dw0381() {
    let bad = QUESTS_V06.replace(
        "\"anchor/door\", \"anchor/exit\"",
        "\"anchor/nowhere\", \"anchor/exit\"",
    );
    assert!(
        diags_for(&bad).iter().any(|d| d.code == "DW0381"),
        "a waypoint anchor no prefab provides is DW0381"
    );
}

#[test]
fn a_repeated_consecutive_waypoint_is_dw0381() {
    let bad = QUESTS_V06.replace(
        "\"anchor/door\", \"anchor/exit\"",
        "\"anchor/exit\", \"anchor/exit\"",
    );
    assert!(
        diags_for(&bad).iter().any(|d| d.code == "DW0381"),
        "marching to where the squad already stands is DW0381"
    );
}

#[test]
fn an_out_of_range_aggro_radius_is_dw0381() {
    let bad = QUESTS_V06.replace("\"aggro_radius\": 16", "\"aggro_radius\": 200");
    assert!(
        diags_for(&bad)
            .iter()
            .any(|d| d.code == "DW0381" && d.path == "/content/waves/0/lane/aggro_radius"),
        "an aggro_radius outside 4..=64 is DW0381"
    );
}

/// The release radius and the mobs' perception radius must be one number: a
/// patrolling raider that targets a player it cannot engage holds ground and the
/// squad stalls mid-lane. A contradicting override is rejected rather than
/// silently overwritten.
#[test]
fn a_contradicting_follow_range_override_is_dw0381() {
    let bad = QUESTS_V06.replace(
        "{ \"entity\": \"minecraft:vindicator\", \"count\": 1 }",
        "{ \"entity\": \"minecraft:vindicator\", \"count\": 1, \"attributes\": { \"follow_range\": 32 } }",
    );
    assert!(
        diags_for(&bad).iter().any(|d| d.code == "DW0381"),
        "a follow_range that disagrees with aggro_radius is DW0381"
    );
}

/// A lane IS routing; aggro-edge is its opposite. Declaring both on one wave is
/// not a blend, it is a contradiction.
#[test]
fn a_lane_plus_aggro_edge_wave_is_dw0381() {
    let bad = QUESTS_V06.replace(
        "\"anchor\": \"anchor/keeper-stand\",",
        "\"anchor\": \"anchor/keeper-stand\",\n        \"summon\": \"aggro-edge\",",
    );
    assert!(
        diags_for(&bad)
            .iter()
            .any(|d| d.code == "DW0381" && d.path == "/content/waves/0/summon"),
        "lane + aggro-edge on one wave is DW0381"
    );
}

// --- DW0382 / DW0383 / DW0384 / DW0385 ------------------------------------

/// `Patrolling`/`patrol_target` are Raider NBT: on anything else they are simply
/// dropped and the mob stands where it spawned — the silent no-op class.
#[test]
fn a_non_raider_lane_species_is_dw0382() {
    let bad = QUESTS_V06.replace("\"minecraft:vindicator\"", "\"minecraft:zombie\"");
    assert!(
        diags_for(&bad)
            .iter()
            .any(|d| d.code == "DW0382" && d.path == "/content/waves/0/mobs/1/entity"),
        "a zombie cannot march a lane: DW0382"
    );
}

/// Every species the spike verified marching is accepted — the roster is a
/// live-verified list, not a guess.
#[test]
fn every_verified_raider_is_accepted_on_a_lane() {
    for species in [
        "minecraft:pillager",
        "minecraft:vindicator",
        "minecraft:evoker",
        "minecraft:ravager",
        "minecraft:witch",
    ] {
        let q = QUESTS_V06.replace("\"minecraft:vindicator\"", &format!("\"{species}\""));
        let diags = diags_for(&q);
        assert!(
            !diags.iter().any(|d| d.code == "DW0382"),
            "{species} is raider-family and must be accepted: {diags:#?}"
        );
    }
}

/// A lone patroller sets `Patrolling:0b` on itself when it finds no companion —
/// vanilla behaviour, so a one-mob lane cancels its own routing.
#[test]
fn a_lone_lane_mob_is_dw0383() {
    let bad = QUESTS_V06.replace(
        "{ \"entity\": \"minecraft:pillager\", \"count\": 2 },\n          ",
        "",
    );
    assert!(
        diags_for(&bad)
            .iter()
            .any(|d| d.code == "DW0383" && d.path == "/content/waves/0/mobs"),
        "a one-mob lane is DW0383"
    );
}

/// A pillager's only attack goal is crossbow-gated: take the crossbow away and it
/// freezes on target acquisition, patrol blocked by the very target it cannot
/// hit. The compiler arms it by default, so this fires only on an explicit
/// override — which is exactly the remaining way into the deadlock.
#[test]
fn a_lane_pillager_without_its_crossbow_is_dw0384() {
    let bad = QUESTS_V06.replace(
        "{ \"entity\": \"minecraft:pillager\", \"count\": 2 }",
        "{ \"entity\": \"minecraft:pillager\", \"count\": 2, \"equipment\": { \"main_hand\": \"minecraft:stick\" } }",
    );
    assert!(
        diags_for(&bad).iter().any(|d| d.code == "DW0384"),
        "a crossbow-less lane pillager is DW0384"
    );
    // The default arming path is NOT a violation: an unspecified main hand takes
    // the compiler's crossbow.
    assert!(
        !diags_for(QUESTS_V06).iter().any(|d| d.code == "DW0384"),
        "the default-armed pillager is fine"
    );
}

/// The ring radius is authored, never guessed: the compiler will not fabricate a
/// vanilla `follow_range` default it cannot verify against the pinned server.
#[test]
fn an_aggro_edge_mob_without_follow_range_is_dw0385() {
    let bad = QUESTS_V06.replace(", \"attributes\": { \"follow_range\": 12 }", "");
    assert!(
        diags_for(&bad)
            .iter()
            .any(|d| d.code == "DW0385" && d.path == "/content/waves/1/mobs/0/attributes"),
        "an aggro-edge mob with no follow_range is DW0385"
    );
}

/// A non-raider is perfectly legal on an aggro-edge wave — that is what the mode
/// exists for. The species rule is scoped to lanes only.
#[test]
fn aggro_edge_accepts_any_species() {
    let diags = diags_for(QUESTS_V06);
    assert!(
        !diags.iter().any(|d| d.code == "DW0382"),
        "the drowned need no patrol AI to be summoned at the edge: {diags:#?}"
    );
}
