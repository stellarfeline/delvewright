//! DSL v0.6: `move-npc` gains `on_arrive` effect hooks — exact parity with
//! `move-actor.on_arrive` (round-6 staging primitives).
//!
//! Owner motivation (island QA round 6): Eurylochus arrived at the hiding alcove
//! AFTER the cutscene because content could only fire-and-forget an NPC walk.
//! `on_arrive` lets content gate a beat on walk completion (`on_arrive` →
//! `set-flag`).
//!
//! Contract under test:
//! * validates clean;
//! * every deep effect walker recurses into it: a `set-flag` nested in
//!   `move-npc.on_arrive` COUNTS as a flag producer (no spurious `DW0172`), and
//!   a nested narrate enters the l10n inventory under the `…​.arrive.<j>.…` key;
//! * a `sequence` reached through `move-npc.on_arrive` inside another `sequence`
//!   is `DW0329`, exactly as through `move-actor.on_arrive`;
//! * the stable `Debug` rendering: an effect using none of the new fields prints
//!   byte-identically to the pre-addition derive (`payload_verb_key`'s content key).

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, parse_campaign};
use std::sync::LazyLock;

/// A v0.6 stage-5 quests doc: the keeper walks to the exit; arrival sets
/// `flag/arrived` (+ narrates), and the follow-up objective is gated on it.
static QUESTS_ARRIVE: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
  "campaign_id": "hello-world",
  "stage": "quests",
  "content": {
    "quests": [
      {
        "id": "quest/open-the-door",
        "trigger": { "type": "campaign-start" },
        "objectives": [
          { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
          { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
            "after": ["obj/talk"], "requires_flags": ["flag/arrived"] }
        ],
        "on_objective_complete": {
          "obj/talk": [
            { "type": "open-gate", "anchor": "anchor/door" },
            { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit",
              "on_arrive": [
                { "type": "set-flag", "flag": "flag/arrived" },
                { "type": "narrate", "text": "The keeper beckons from the doorway." }
              ] }
          ]
        },
        "on_complete": [ { "type": "campaign-complete" } ]
      }
    ]
  }
}"#,
    )
});

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
        design: None,
    }
}

/// `move-npc.on_arrive` validates clean under 0.6.0 — and the nested `set-flag`
/// counts as the producer for the flag-gated objective (no `DW0172`): the
/// consumer-validation walkers recurse into the new nesting site.
#[test]
fn move_npc_on_arrive_validates_clean_and_produces_flags() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_ARRIVE.as_str()));
    assert!(
        diags.is_empty(),
        "move-npc.on_arrive must validate clean (nested set-flag is a producer): {diags:#?}"
    );
}

/// A `sequence` reached through `move-npc.on_arrive` inside another `sequence`
/// recurses a timeline → `DW0329` (parity with the `move-actor` rule).
#[test]
fn sequence_via_move_npc_on_arrive_inside_sequence_is_dw0329() {
    let quests = QUESTS_ARRIVE.replace(
        r#"{ "type": "set-flag", "flag": "flag/arrived" }"#,
        r#"{ "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [ { "type": "set-flag", "flag": "flag/arrived" } ] } ] }"#,
    );
    // Wrap the move-npc itself in a sequence step: outer sequence -> move-npc
    // .on_arrive -> inner sequence.
    let quests = quests.replace(
        r#"{ "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit","#,
        r#"{ "type": "sequence", "steps": [ { "at_ticks": 0, "effects": [ { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit","#,
    );
    let quests = quests.replace(
        r#"{ "type": "narrate", "text": "The keeper beckons from the doorway." }
              ] }"#,
        r#"{ "type": "narrate", "text": "The keeper beckons from the doorway." }
              ] } ] } ] }"#,
    );
    let diags = check_campaign(&campaign_with_quests(&quests));
    assert!(
        diags.iter().any(|d| d.code == "DW0329"),
        "a sequence via move-npc.on_arrive inside a sequence must be DW0329: {diags:#?}"
    );
}

/// A narrate nested in `move-npc.on_arrive` enters the l10n inventory under the
/// position-derived `…​.arrive.<j>.narrate` key — the emission/localization path
/// shares the same traversal, so a translated build localizes it too.
#[test]
fn move_npc_on_arrive_narrate_enters_l10n_inventory() {
    let campaign =
        parse_campaign(&campaign_with_quests(QUESTS_ARRIVE.as_str())).expect("campaign parses");
    let inv = delvewright_dsl::l10n_inventory(&campaign);
    let key = "fx.open-the-door.oc.talk.1.arrive.1.narrate";
    assert_eq!(
        inv.get(key).map(String::as_str),
        Some("The keeper beckons from the doorway."),
        "narrate inside move-npc.on_arrive must be inventoried under `{key}`; got keys: {:#?}",
        inv.keys().collect::<Vec<_>>()
    );
}
