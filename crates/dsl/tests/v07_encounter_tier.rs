//! DSL v0.7 wave `tier` (spec-0023): what the content *bills* an encounter as.
//!
//! The field exists for one consumer — the validation ladder's inverted floor
//! gate, which reports an `elite`/`boss` fight the unassisted bot beats on its
//! first try as too easy for its billing. It is a declaration, never a knob: the
//! compiler emits identical gameplay whichever tier a wave carries, so declaring
//! one can only ever change validation metadata.
//!
//! Marking is authored rather than inferred on purpose. "This stack looks tuned,
//! so it must be an elite" is exactly the downstream folklore CLAUDE.md's
//! no-hack rule forbids; when content needs to say something, the DSL gets a
//! word for it.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// hello-world with a `waves` section whose single wave carries `tier`, at the
/// given quests-stage `dsl_version`.
fn quests_with_tier(tier: &str, version: &str) -> String {
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
          {{ "type": "kill", "id": "obj/slay", "wave": "wave/guards", "after": ["obj/talk"] }},
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
             "radius": 2, "after": ["obj/slay"] }}
        ],
        "on_objective_complete": {{
          "obj/talk": [
            {{ "type": "open-gate", "anchor": "anchor/door" }},
            {{ "type": "spawn-wave", "wave": "wave/guards" }}
          ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "waves": [
      {{ "id": "wave/guards", "anchor": "anchor/keeper-stand",
         "mobs": [{{ "entity": "minecraft:zombie", "count": 1 }}]{tier} }}
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
    }
}

#[test]
fn every_tier_keyword_validates_at_v07() {
    for tier in ["ordinary", "elite", "boss"] {
        let raw = raw_with_quests(quests_with_tier(
            &format!(",\n         \"tier\": \"{tier}\""),
            "0.7.0",
        ));
        let d = check_campaign(&raw);
        assert!(d.is_empty(), "`{tier}` must validate clean: {d:#?}");
    }
}

#[test]
fn a_tier_below_v07_is_dw0141() {
    let raw = raw_with_quests(quests_with_tier(",\n         \"tier\": \"boss\"", "0.6.0"));
    let d = check_campaign(&raw);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0141")
        .expect("a pre-0.7 `tier` is reserved surface");
    assert_eq!(hit.stage, "quests");
    assert_eq!(hit.path, "/content/waves/0/tier");
    assert!(hit.message.contains("0.7.0"), "{}", hit.message);
}

#[test]
fn an_absent_tier_is_clean_at_any_version() {
    // The whole byte-identity argument: every campaign written before this field
    // existed keeps validating exactly as it did.
    for version in ["0.3.0", "0.6.0", "0.7.0"] {
        let raw = raw_with_quests(quests_with_tier("", version));
        let d = check_campaign(&raw);
        assert!(
            d.iter().all(|x| x.code != "DW0141"),
            "no tier means no reserved-surface finding at {version}: {d:#?}"
        );
    }
}
