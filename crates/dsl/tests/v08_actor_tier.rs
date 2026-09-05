//! DSL v0.8 actor `tier` (spec-0023): the OTHER shape an elite takes.
//!
//! `waves[].tier` (v0.7) only ever described one implementation of a hard fight.
//! The set-piece souls encounter — the armoured thing kneeling among the graves
//! that stands up when you strike it — is a stage-5 **actor**, staged by
//! `spawn-actor` and given AI by `unleash-actor`, and it was structurally
//! invisible to the validation ladder's inverted floor gate: an empty finding
//! list read as a pass while covering nothing.
//!
//! Same contract as the wave field, one version later: declaring it below 0.8.0
//! absence is byte-identical to a campaign that declares none,
//! and it never reaches emission.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// hello-world's quests stage with one actor, optionally tiered, at the given
/// quests-stage `dsl_version`.
fn quests_with_actor_tier(tier: &str, version: &str) -> String {
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
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "spawn-actor", "actor": "actor/warden" }},
            {{ "type": "unleash-actor", "actor": "actor/warden" }}
          ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "actors": [
      {{ "id": "actor/warden", "entity": "minecraft:wither_skeleton",
         "anchor": "anchor/keeper-stand"{tier} }}
    ]
  }}
}}"#
    )
}

fn raw_with_quests(quests: String) -> RawCampaign {
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

#[test]
fn every_tier_keyword_validates_at_v08() {
    for tier in ["ordinary", "elite", "boss"] {
        let raw = raw_with_quests(quests_with_actor_tier(
            &format!(",\n         \"tier\": \"{tier}\""),
            "0.19.0",
        ));
        let d = check_campaign(&raw);
        // `DW0469` is the fixture's own pre-existing advisory (a fighting actor
        // in a campaign that declares no `world.difficulty`) and has nothing to
        // do with the tier — it fires identically with the field absent.
        assert!(
            d.iter().all(|x| x.code == "DW0469"),
            "`{tier}` must validate clean: {d:#?}"
        );
    }
}
