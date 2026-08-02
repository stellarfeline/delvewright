//! NPC location-continuity lint (`DW0351`, warning tier) — round-6 staging
//! primitives.
//!
//! Reproduces the island QA discontinuities in miniature and proves the
//! conservative model's negative space:
//!
//! * *unstaged entrance* (the `npc/perimedes` shape): a deferred NPC spawned
//!   mid-story with no staged arrival warns; the walked hand-off (`spawn-npc`
//!   inside a move's `on_arrive` arriving at the NPC's anchor — the
//!   `npc/polyphemus` shape) does not;
//! * *remote dismissal* (the `npc/antiphos` shape): a despawn fired from a beat
//!   staged at another anchor warns; the walk-then-despawn-in-scene shape (the
//!   `v04-showcase` keeper) does not;
//! * *re-entry jump*: respawning an NPC at its declared anchor after it was
//!   last staged elsewhere warns;
//! * *exclusion*: an NPC whose lifecycle is touched from a trigger (no DAG
//!   position) is untracked — no warning, however discontinuous.

mod common;

use delvewright_compiler::continuity::check_npc_continuity;
use delvewright_dsl::{Campaign, RawCampaign, Severity, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world's npcs doc at 0.6.0, optionally with the keeper `deferred`.
fn npcs_doc(deferred: bool) -> String {
    let src = read_hw("npcs.json").replacen("\"0.2.0\"", "\"0.6.0\"", 1);
    if deferred {
        src.replace(
            "\"base_entity\": \"minecraft:villager\",",
            "\"base_entity\": \"minecraft:villager\",\n        \"deferred\": true,",
        )
    } else {
        src
    }
}

fn parse(deferred: bool, quests: &str) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: npcs_doc(deferred),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: read_hw("dialogue.json"),
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// A v0.6 quests doc skeleton; `{TALK_FX}` and `{COMPLETE_FX}` are the
/// `obj/talk` completion bundle and the quest `on_complete` bundle.
fn quests(talk_fx: &str, complete_fx: &str) -> String {
    r#"{
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
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ {TALK_FX} ]
        },
        "on_complete": [ {COMPLETE_FX} { "type": "campaign-complete" } ]
      }
    ]
  }
}"#
    .replacen("{TALK_FX}", talk_fx, 1)
    .replacen("{COMPLETE_FX}", complete_fx, 1)
}

fn dw0350(c: &Campaign) -> Vec<delvewright_dsl::Diagnostic> {
    let diags = check_npc_continuity(c);
    for d in &diags {
        assert_eq!(d.code, "DW0351");
        assert_eq!(d.severity, Severity::Warning, "DW0351 is warning-tier");
    }
    diags
}

/// The perimedes shape: a deferred NPC spawned mid-story straight into the
/// world — never staged entering — warns, naming npc, anchor and remedy.
#[test]
fn deferred_spawn_with_no_staged_entrance_warns_dw0350() {
    let c = parse(
        true,
        &quests(r#"{ "type": "spawn-npc", "npc": "npc/keeper" }"#, ""),
    );
    let diags = dw0350(&c);
    assert_eq!(diags.len(), 1, "exactly one warning: {diags:#?}");
    let msg = &diags[0].message;
    assert!(
        msg.contains("npc/keeper")
            && msg.contains("anchor/keeper-stand")
            && msg.contains("never been staged entering")
            && msg.contains("on_arrive"),
        "message names the discontinuity and the walked-hand-off remedy: {msg}"
    );
}

/// The polyphemus shape: the same deferred spawn fired from a `move-actor`
/// `on_arrive` whose destination IS the NPC's anchor is a staged entrance — no
/// warning.
#[test]
fn deferred_spawn_covered_by_arrival_is_clean() {
    let quests = quests(
        r#"{ "type": "spawn-actor", "actor": "actor/stand-in" },
            { "type": "move-actor", "actor": "actor/stand-in", "to_anchor": "anchor/keeper-stand",
              "on_arrive": [
                { "type": "despawn-actor", "actor": "actor/stand-in", "style": "vanish" },
                { "type": "spawn-npc", "npc": "npc/keeper" }
              ] }"#,
        "",
    )
    .replacen(
        r#""quests": ["#,
        r#""actors": [ { "id": "actor/stand-in", "entity": "minecraft:villager", "anchor": "anchor/door" } ],
    "quests": ["#,
        1,
    );
    let c = parse(true, &quests);
    assert!(
        dw0350(&c).is_empty(),
        "a walked hand-off arriving at the NPC's anchor is a staged entrance"
    );
}

/// The antiphos shape: despawning an NPC from a beat staged at another anchor
/// (`obj/exit` plays at `anchor/exit`; the keeper stands at
/// `anchor/keeper-stand`) warns as a remote dismissal.
#[test]
fn remote_despawn_warns_dw0350() {
    let c = parse(
        false,
        &quests("", r#"{ "type": "despawn-npc", "npc": "npc/keeper" },"#),
    );
    let diags = dw0350(&c);
    assert_eq!(diags.len(), 1, "exactly one warning: {diags:#?}");
    let msg = &diags[0].message;
    assert!(
        msg.contains("npc/keeper")
            && msg.contains("anchor/keeper-stand")
            && msg.contains("anchor/exit")
            && msg.contains("move-npc"),
        "message names both anchors and the walk remedy: {msg}"
    );
}

/// The v04-showcase keeper shape: walk the NPC to the scene first, then despawn
/// it there — continuous, no warning.
#[test]
fn walk_then_despawn_in_scene_is_clean() {
    let c = parse(
        false,
        &quests(
            r#"{ "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }"#,
            r#"{ "type": "despawn-npc", "npc": "npc/keeper" },"#,
        ),
    );
    assert!(
        dw0350(&c).is_empty(),
        "moved-to-scene then despawned-in-scene is continuous"
    );
}

/// A re-entry jump: despawn in scene, then spawn the NPC back at its declared
/// anchor (across the map from where players saw it leave) warns.
#[test]
fn respawn_away_from_last_staged_location_warns_dw0350() {
    let c = parse(
        false,
        &quests(
            r#"{ "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }"#,
            r#"{ "type": "despawn-npc", "npc": "npc/keeper" },
               { "type": "spawn-npc", "npc": "npc/keeper" },"#,
        ),
    );
    let diags = dw0350(&c);
    assert_eq!(diags.len(), 1, "exactly one warning: {diags:#?}");
    let msg = &diags[0].message;
    assert!(
        msg.contains("re-materializes")
            && msg.contains("anchor/keeper-stand")
            && msg.contains("anchor/exit"),
        "message names the jump endpoints: {msg}"
    );
}

/// Conservative exclusion: the same discontinuous history is silenced when the
/// NPC is also staged from an environment trigger — its timeline has no static
/// order, so the lint must not guess.
#[test]
fn trigger_touched_npc_is_excluded() {
    let quests = quests("", r#"{ "type": "despawn-npc", "npc": "npc/keeper" },"#).replacen(
        r#""quests": ["#,
        r#""triggers": [
      { "id": "trigger/return", "at": "anchor/door", "on": { "on": "approach", "range": 3 },
        "effects": [ { "type": "spawn-npc", "npc": "npc/keeper" } ] }
    ],
    "quests": ["#,
        1,
    );
    let c = parse(false, &quests);
    assert!(
        dw0350(&c).is_empty(),
        "an NPC staged from a trigger is untracked (conservative model)"
    );
}
