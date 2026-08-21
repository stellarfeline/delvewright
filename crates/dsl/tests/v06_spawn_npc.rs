//! DSL v0.6 `deferred` NPCs + the `spawn-npc` effect (the dual of `despawn-npc`):
//! validates under `dsl_version 0.6.0` and is reserved (`DW0141`) earlier; an
//! unknown npc ref is `DW0112`; a deferred npc nobody spawns is `DW0197`; a
//! `talk-to` on a deferred npc every spawn provably follows is `DW0198`.
//!
//! Built on the hello-world fixture, extended with a second quest (`quest/second`,
//! depending on the first) and a second, deferred NPC.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// Stage 2 with a deferred second NPC.
const NPCS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "npcs",
  "content": {
    "npcs": [
      {
        "id": "npc/keeper",
        "name": "The Keeper",
        "role": "quest-giver",
        "area": "area/keep",
        "anchor": "anchor/keeper-stand",
        "base_entity": "minecraft:villager",
        "persona": {
          "archetype": "stoic gatekeeper",
          "speech_style": "Terse and formal.",
          "demeanor": "Wary and unmoving.",
          "motivation": "Keep the gate shut.",
          "secret": "He blames himself for the drowned road.",
          "backstory": "A road-warden who held his post alone."
        }
      },
      {
        "id": "npc/latecomer",
        "name": "The Latecomer",
        "role": "flavor",
        "area": "area/keep",
        "anchor": "anchor/exit",
        "base_entity": "minecraft:villager",
        "deferred": true,
        "persona": {
          "archetype": "hooded stranger",
          "speech_style": "Low and hurried.",
          "demeanor": "Appears only once the door is open.",
          "motivation": "Slip through the gate behind the party.",
          "secret": "She has been waiting outside for years.",
          "backstory": "Turned away from the keep on the night the road drowned."
        }
      }
    ]
  }
}"#;

/// Stage 4: two quests, `quest/second` depending on `quest/open-the-door`.
const QUEST_PLAN: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quest-plan",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "goal": "Get the Keeper to open the door.",
        "area": "area/keep",
        "npcs": ["npc/keeper"],
        "depends_on": [],
        "mandatory": true,
        "act": 1
      },
      {
        "id": "quest/second",
        "goal": "Meet whoever was waiting outside.",
        "area": "area/keep",
        "npcs": ["npc/latecomer"],
        "depends_on": ["quest/open-the-door"],
        "mandatory": true,
        "act": 1
      }
    ],
    "finale": "quest/second"
  }
}"#;

/// Stage 6: a tree per NPC (`DW0152`), the latecomer's completing its `talk-to`.
const DIALOGUE: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {
    "dialogues": [
      {
        "npc": "npc/keeper",
        "root": "dlg/greeting",
        "nodes": [
          {
            "id": "dlg/greeting",
            "text": "Halt, traveler. The door stays shut.",
            "options": [
              { "label": "Open the door, please.", "effects": [ { "type": "complete-objective", "objective": "obj/talk" } ] }
            ]
          }
        ]
      },
      {
        "npc": "npc/latecomer",
        "root": "dlg/hello",
        "nodes": [
          {
            "id": "dlg/hello",
            "text": "You opened it. I have waited a long time for that.",
            "options": [
              { "label": "Who are you?", "effects": [ { "type": "complete-objective", "objective": "obj/meet" } ] }
            ]
          }
        ]
      }
    ]
  }
}"#;

/// Stage 5: quest 1 spawns the latecomer when the door opens; quest 2 talks to her.
const QUESTS: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "spawn-npc", "npc": "npc/latecomer" }
          ]
        },
        "on_complete": []
      },
      {
        "id": "quest/second",
        "trigger": { "type": "quest-complete", "quest": "quest/open-the-door" },
        "objectives": [
          { "type": "talk-to", "id": "obj/meet", "npc": "npc/latecomer" }
        ],
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#;

fn campaign(npcs: &str, quests: &str, dialogue: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: npcs.to_string(),
        classes: common::read_valid("classes.json"),
        quest_plan: QUEST_PLAN.to_string(),
        quests: quests.to_string(),
        dialogue: dialogue.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
    }
}

fn base() -> RawCampaign {
    campaign(NPCS, QUESTS, DIALOGUE)
}

/// A deferred NPC with a `spawn-npc` in a prerequisite quest validates clean.
#[test]
fn deferred_npc_with_spawn_validates_clean() {
    let diags = check_campaign(&base());
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 deferred npc + spawn-npc, got: {diags:#?}"
    );
}

/// `deferred` under a pre-0.6 npcs stage is reserved → `DW0141`.
#[test]
fn deferred_reserved_before_0_6() {
    let pre = NPCS.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign(&pre, QUESTS, DIALOGUE));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "npc `deferred` must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// `spawn-npc` under a pre-0.6 quests stage is reserved → `DW0141`.
#[test]
fn spawn_npc_effect_reserved_before_0_6() {
    let pre = QUESTS.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign(NPCS, &pre, DIALOGUE));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "`spawn-npc` must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// `spawn-npc` on an npc stage 2 never declares joins the `DW0112` dangling-ref
/// family (same as `despawn-npc`/`move-npc`).
#[test]
fn spawn_npc_unknown_npc_is_dw0112() {
    let bad = QUESTS.replace(
        r#"{ "type": "spawn-npc", "npc": "npc/latecomer" }"#,
        r#"{ "type": "spawn-npc", "npc": "npc/nobody" }"#,
    );
    let diags = check_campaign(&campaign(NPCS, &bad, DIALOGUE));
    assert!(
        diags.iter().any(|d| d.code == "DW0112"),
        "an unknown spawn-npc target must be DW0112: {diags:#?}"
    );
}

/// A dialogue `spawn-npc` on an unknown npc is also `DW0112`.
#[test]
fn dialogue_spawn_npc_unknown_npc_is_dw0112() {
    let bad_dlg = DIALOGUE.replace(
        r#"{ "label": "Who are you?", "effects": [ { "type": "complete-objective", "objective": "obj/meet" } ] }"#,
        r#"{ "label": "Who are you?", "effects": [ { "type": "complete-objective", "objective": "obj/meet" }, { "type": "spawn-npc", "npc": "npc/nobody" } ] }"#,
    );
    let diags = check_campaign(&campaign(NPCS, QUESTS, &bad_dlg));
    assert!(
        diags.iter().any(|d| d.code == "DW0112"),
        "an unknown dialogue spawn-npc target must be DW0112: {diags:#?}"
    );
}

/// A deferred NPC that no `spawn-npc` anywhere summons is unreachable content →
/// `DW0197`.
#[test]
fn deferred_npc_never_spawned_is_dw0197() {
    let no_spawn = QUESTS.replace(
        r#",
            { "type": "spawn-npc", "npc": "npc/latecomer" }"#,
        "",
    );
    assert!(
        !no_spawn.contains("spawn-npc"),
        "fixture edit must remove the spawn"
    );
    let diags = check_campaign(&campaign(NPCS, &no_spawn, DIALOGUE));
    assert!(
        diags.iter().any(|d| d.code == "DW0197"),
        "a deferred npc nobody spawns must be DW0197: {diags:#?}"
    );
}

/// A `talk-to` on a deferred NPC whose only `spawn-npc` fires in a quest that
/// *depends on* the objective's quest activates on an empty anchor → `DW0198`.
#[test]
fn talk_to_before_spawn_in_dag_is_dw0198() {
    // Move the spawn into `quest/second` (the descendant) — the `talk-to` on the
    // latecomer lives there too, but the spawn is on `on_complete`, i.e. only after
    // the whole quest (including the talk-to) is done... to make the DAG order
    // unambiguous, put the talk-to in quest 1 and the spawn in quest 2.
    let swapped = r#"{
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
          { "type": "talk-to", "id": "obj/meet", "npc": "npc/latecomer", "after": ["obj/talk"] }
        ],
        "on_objective_complete": {
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
        },
        "on_complete": []
      },
      {
        "id": "quest/second",
        "trigger": { "type": "quest-complete", "quest": "quest/open-the-door" },
        "objectives": [
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2 }
        ],
        "on_complete": [
          { "type": "spawn-npc", "npc": "npc/latecomer" },
          { "type": "campaign-complete" }
        ]
      }
    ]
  }
}"#;
    let diags = check_campaign(&campaign(NPCS, swapped, DIALOGUE));
    assert!(
        diags.iter().any(|d| d.code == "DW0198"),
        "a talk-to that provably precedes every spawn-npc must be DW0198: {diags:#?}"
    );
}
