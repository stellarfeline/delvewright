//! DSL v0.8 `close-gate.sealed_hint`: the
//! line a sealed gate answers a right-click with. Validates under
//! `dsl_version 0.8.0`, is reserved (`DW0141`) earlier, enters the l10n inventory
//! when authored (and only then), and never moves a content key by existing.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, l10n_inventory, parse_campaign};

/// A v0.8 quests document that seals `anchor/door` with an authored answer.
const QUESTS_V08: &str = r#"{
  "dsl_version": "0.19.0",
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
          "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door",
                          "happening": { "verb": "opens", "text": "The bars lift." } } ]
        },
        "on_complete": [
          { "type": "close-gate", "anchor": "anchor/door",
            "sealed_hint": "The bars will not lift for you.",
            "happening": { "verb": "seals", "text": "The bars come down." } },
          { "type": "campaign-complete",
            "happening": { "verb": "survives", "text": "The party is out." } }
        ],
        "happening": { "verb": "opens", "text": "The door is dealt with." }
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

/// An authored `sealed_hint` validates clean under `dsl_version 0.8.0`.
#[test]
fn sealed_hint_validates_clean_at_0_8() {
    let diags = check_campaign(&campaign_with_quests(QUESTS_V08));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.8 sealed_hint, got: {diags:#?}"
    );
}

/// An authored hint is a player-visible string, so it enters the l10n inventory
/// under the sealing effect's own position-derived key and translates like a
/// `narrate` line.
#[test]
fn an_authored_hint_is_inventoried() {
    let c = parse_campaign(&campaign_with_quests(QUESTS_V08)).expect("campaign parses");
    let inv = l10n_inventory(&c);
    assert_eq!(
        inv.get("fx.open-the-door.done.0.sealed_hint")
            .map(|s| s.as_str()),
        Some("The bars will not lift for you."),
        "the seal's answer must be translatable: {inv:#?}"
    );
}

/// An unauthored hint is absent from the inventory: the compiler bakes its
/// canonical English there, exactly as `world.boundary.message` does, so a
/// translator is never handed a key the campaign did not write.
#[test]
fn an_unauthored_hint_is_not_inventoried() {
    let plain = QUESTS_V08.replace(
        "\"sealed_hint\": \"The bars will not lift for you.\",\n            ",
        "",
    );
    let c = parse_campaign(&campaign_with_quests(&plain)).expect("campaign parses");
    assert!(
        !l10n_inventory(&c)
            .keys()
            .any(|k| k.ends_with("sealed_hint")),
        "an unauthored seal answer must not appear in the inventory"
    );
}

/// A `close-gate` that authors no hint renders in `Debug` exactly as it did
/// before the field existed — the content-key stability rule, so no existing
/// campaign's generated `seq_<hash>` function names move.
#[test]
fn an_unauthored_hint_does_not_move_a_content_key() {
    use delvewright_dsl::{AnchorId, QuestEffect};
    let plain = QuestEffect::CloseGate {
        anchor: AnchorId("anchor/door".to_string()),
        requires_flags: Vec::new(),
        forbids_flags: Vec::new(),
        requires_state: Vec::new(),
        happening: None,
        sealed_hint: None,
    };
    assert_eq!(
        format!("{plain:?}"),
        "CloseGate { anchor: AnchorId(\"anchor/door\"), requires_flags: [] }"
    );
    let authored = QuestEffect::CloseGate {
        anchor: AnchorId("anchor/door".to_string()),
        requires_flags: Vec::new(),
        forbids_flags: Vec::new(),
        requires_state: Vec::new(),
        happening: None,
        sealed_hint: Some("It will not shift.".to_string()),
    };
    assert_ne!(
        format!("{authored:?}"),
        format!("{plain:?}"),
        "an authored answer changes emission, so it must change the content key"
    );
}
