//! DSL v0.3: a full campaign exercising every new stage-5 verb
//! (`kill`/`collect`/`interact`) and effect (`spawn-wave`/`give-item`/`set-flag`)
//! plus `requires_flags` gating validates with zero diagnostics, and the new
//! types round-trip canonically.
//!
//! Built on the hello-world casting/world/dialogue (bumped to 0.3.0) with a
//! verb-rich stage-5 replacement, so it resolves against the vendored registries
//! (hello-room anchors, the item/entity subsets).

mod common;

use delvewright_dsl::{Envelope, QuestsContent, RawCampaign, check_campaign, to_canonical_string};

/// Stage 5 exercising all v0.3 verbs, waves and a flag causal chain.
const QUESTS_V03: &str = r#"{
  "dsl_version": "0.3.0",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "kill", "id": "obj/slay", "wave": "wave/guards", "after": ["obj/talk"] },
          { "type": "collect", "id": "obj/gather", "item": "minecraft:bread", "count": 1, "anchor": "anchor/exit", "after": ["obj/slay"] },
          { "type": "interact", "id": "obj/unlock", "anchor": "anchor/door", "requires_item": "minecraft:iron_sword", "after": ["obj/gather"], "requires_flags": ["flag/armed"] },
          { "type": "reach-anchor", "id": "obj/reach", "anchor": "anchor/exit", "radius": 2, "after": ["obj/unlock"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "spawn-wave", "wave": "wave/guards" },
            { "type": "set-flag", "flag": "flag/armed" }
          ],
          "obj/slay": [
            { "type": "give-item", "item": "minecraft:torch", "count": 1 }
          ],
          "obj/unlock": [
            { "type": "open-gate", "anchor": "anchor/door" }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ],
    "waves": [
      { "id": "wave/guards", "anchor": "anchor/keeper-stand", "mobs": [ { "entity": "minecraft:zombie", "count": 2, "name": "Keep Guard" } ] }
    ]
  }
}"#;

/// Re-stamp a stage document's `dsl_version` to 0.3.0.
fn to_v03(doc: &str) -> String {
    doc.replacen(
        "\"dsl_version\": \"0.2.0\"",
        "\"dsl_version\": \"0.3.0\"",
        1,
    )
}

fn v03_campaign() -> RawCampaign {
    RawCampaign {
        world: to_v03(&common::read_valid("world.json")),
        npcs: to_v03(&common::read_valid("npcs.json")),
        classes: to_v03(&common::read_valid("classes.json")),
        quest_plan: to_v03(&common::read_valid("quest-plan.json")),
        quests: QUESTS_V03.to_string(),
        dialogue: to_v03(&common::read_valid("dialogue.json")),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
    }
}

#[test]
fn v03_verbs_campaign_validates_clean() {
    let diags = check_campaign(&v03_campaign());
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.3 verbs campaign, got: {diags:#?}"
    );
}

#[test]
fn v03_quests_canonical_roundtrip_is_idempotent() {
    let env: Envelope<QuestsContent> = serde_json::from_str(QUESTS_V03).expect("parse v0.3 quests");
    let once = to_canonical_string(&env).expect("canonical serialize");
    let reparsed: Envelope<QuestsContent> =
        serde_json::from_str(&once).expect("re-parse canonical");
    let twice = to_canonical_string(&reparsed).expect("canonical serialize again");
    assert_eq!(
        once, twice,
        "canonical writer is not idempotent for v0.3 types"
    );
}
