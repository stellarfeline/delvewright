//! `on: strike-npc` — the body-targeting trigger form (DSL v0.6).
//!
//! The owner's island round-7 finding #1: "strike the giant" was authored as a
//! `strike` trigger at a world anchor, which summons its own interaction entity
//! at a *cell* — and the giant's 0.9 × 2.9 body eclipses that cell (`DW0359`),
//! so the click never reached the trigger. `strike-npc` has no cell: it names
//! the NPC and rides the interaction entity that NPC already owns, which is the
//! entity a click on that NPC reaches by construction.
//!
//! These tests pin the DSL surface: `at` and `strike-npc` are mutually
//! exclusive (`DW0194` either way), the target must be a declared NPC
//! (`DW0112`), and the whole form is a v0.6 surface (`DW0141` before that).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

fn campaign_with(quests: &str) -> RawCampaign {
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

/// A quests document at `version` carrying one trigger, spelled by the caller.
fn quests_with(version: &str, trigger: &str) -> String {
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
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {{ "type": "campaign-complete" }} ]
      }}
    ],
    "triggers": [ {trigger} ]
  }}
}}"#
    )
}

const NARRATE: &str = r#"[ { "type": "narrate", "style": "chat", "text": "He roars." } ]"#;

/// The intended spelling: no `at`, an NPC named in the event.
fn good_trigger() -> String {
    format!(
        r#"{{
      "id": "trigger/wake",
      "on": {{ "on": "strike-npc", "npc": "npc/keeper" }},
      "effects": {NARRATE}
    }}"#
    )
}

/// The canonical form validates clean — no anchor, and none wanted.
#[test]
fn strike_npc_without_an_anchor_is_valid() {
    let diags = check_campaign(&campaign_with(&quests_with("0.19.0", &good_trigger())));
    assert!(
        diags.is_empty(),
        "a `strike-npc` trigger naming a declared NPC must validate clean: {diags:#?}"
    );
}

/// `at` names a place; `strike-npc` names a character. An anchor alongside it
/// would be silently ignored — the class of authoring mistake that reads as
/// meaningful and does nothing — so it is rejected outright.
#[test]
fn strike_npc_with_an_anchor_is_dw0194() {
    let trigger = format!(
        r#"{{
      "id": "trigger/wake",
      "at": "anchor/keeper-stand",
      "on": {{ "on": "strike-npc", "npc": "npc/keeper" }},
      "effects": {NARRATE}
    }}"#
    );
    let diags = check_campaign(&campaign_with(&quests_with("0.19.0", &trigger)));
    assert!(
        diags.iter().any(|d| d.code == "DW0194"),
        "an `at` on a `strike-npc` trigger must be DW0194: {diags:#?}"
    );
}

/// The dual: a place-watching event with no place.
#[test]
fn strike_without_an_anchor_is_dw0194() {
    let trigger = format!(
        r#"{{
      "id": "trigger/ward",
      "on": {{ "on": "strike" }},
      "effects": {NARRATE}
    }}"#
    );
    let diags = check_campaign(&campaign_with(&quests_with("0.19.0", &trigger)));
    assert!(
        diags.iter().any(|d| d.code == "DW0194"),
        "a `strike` trigger with no `at` must be DW0194: {diags:#?}"
    );
}

/// The trigger's tag rides the target NPC's hitbox, so an unknown target would
/// tag nothing and the trigger could never fire — the same dangling-ref code
/// the `move-npc`/`spawn-npc` family raises.
#[test]
fn strike_npc_targeting_an_unknown_npc_is_dw0112() {
    let trigger = format!(
        r#"{{
      "id": "trigger/wake",
      "on": {{ "on": "strike-npc", "npc": "npc/nobody" }},
      "effects": {NARRATE}
    }}"#
    );
    let diags = check_campaign(&campaign_with(&quests_with("0.19.0", &trigger)));
    assert!(
        diags.iter().any(|d| d.code == "DW0112"),
        "an unknown `strike-npc` target must be DW0112: {diags:#?}"
    );
}
