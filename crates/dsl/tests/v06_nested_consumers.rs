//! Nested-effect **consumer** validation recursion, DSL half: an
//! unknown-wave (`DW0170`), unknown-item
//! (`DW0143`) or unknown-block (`DW0193`) reference nested inside a `sequence` step
//! (or a lifecycle bundle) is caught by the deep consumer scan, not shipped
//! unvalidated. The compiler-side sound (`DW0326`) / art-glyph (`DW0328`) halves are
//! in `crates/compiler/tests/v06.rs` (they run in the compiler's validate stage).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 quests doc whose `on_complete` is a single `sequence` wrapping the given
/// effects (a raw JSON array body). The sequence nesting is the recursion probe.
fn quests_with_sequence(seq_effects: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "0.19.0",
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
        "on_complete": [
          {{ "type": "sequence", "steps": [ {{ "at_ticks": 0, "effects": [ {seq_effects} ] }} ] }},
          {{ "type": "campaign-complete" }}
        ]
      }}
    ]
  }}
}}"#
    )
}

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

fn diags_for(seq_effects: &str) -> Vec<delvewright_dsl::Diagnostic> {
    check_campaign(&campaign_with_quests(&quests_with_sequence(seq_effects)))
}

/// A valid `narrate` nested in a sequence validates clean (baseline: the deep scan
/// does not spuriously reject a good nested ref).
#[test]
fn nested_valid_narrate_is_clean() {
    let d = diags_for(r#"{ "type": "narrate", "text": "safe now" }"#);
    assert!(
        d.is_empty(),
        "a valid nested narrate must validate clean: {d:#?}"
    );
}

/// A `spawn-wave` referencing an unknown wave nested in a sequence step fires the
/// existing unknown-wave code `DW0170`.
#[test]
fn nested_unknown_spawn_wave_is_dw0170() {
    let d = diags_for(r#"{ "type": "spawn-wave", "wave": "wave/ghosts" }"#);
    assert!(
        d.iter().any(|x| x.code == "DW0170"),
        "a nested spawn-wave of an unknown wave must fire DW0170: {d:#?}"
    );
}

/// A `give-item` with an unknown item id nested in a sequence step fires `DW0143`.
#[test]
fn nested_unknown_give_item_is_dw0143() {
    let d = diags_for(r#"{ "type": "give-item", "item": "minecraft:not_an_item", "count": 1 }"#);
    assert!(
        d.iter().any(|x| x.code == "DW0143"),
        "a nested give-item of an unknown item must fire DW0143: {d:#?}"
    );
}

/// A `set-block` with an unknown block id nested in a sequence step fires `DW0193`.
#[test]
fn nested_unknown_set_block_is_dw0193() {
    let d = diags_for(
        r#"{ "type": "set-block", "anchor": "anchor/exit", "block": "minecraft:not_a_block" }"#,
    );
    assert!(
        d.iter().any(|x| x.code == "DW0193"),
        "a nested set-block of an unknown block must fire DW0193: {d:#?}"
    );
}
