//! DSL v0.8 dialogue-option `tooltip`: the button keeps
//! a caption, the hover box carries the full line.
//!
//! Ground truth, not folklore. In the pinned 1.21.11 client jar a dialog action
//! button is `ActionButton(CommonButtonData, Optional<DialogAction>)`, and
//! `CommonButtonData`'s `MapCodec` is built from exactly three fields:
//! `Codec.fieldOf("label")`, `Codec.optionalFieldOf("tooltip")` and
//! `Codec.optionalFieldOf("width", 150)`. The client's dialog control set turns a
//! present `tooltip` into `Tooltip.create(component)` and hangs it on the button,
//! and `Tooltip` splits its text with `Font.split(message, 170)` — a hover box
//! that **wraps**. So a tooltip carries a sentence the 146-px button never could,
//! and `DW0331` (which exists to reject *scrolling* captions) has nothing to say
//! about it.
//!
//! Same contract every v0.8 field carries: declaring it below 0.8.0 is `DW0141`,
//! absence is byte-identical to every campaign written before it.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, l10n, parse_campaign};

/// hello-world's dialogue stage at `version`, with `tooltip` spliced into the
/// first option of the root node (or nothing at all when `tooltip` is empty).
fn dialogue_with_tooltip(tooltip: &str, version: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}",
  "campaign_id": "hello-world",
  "stage": "dialogue",
  "content": {{
    "dialogues": [
      {{
        "npc": "npc/keeper",
        "root": "dlg/greeting",
        "nodes": [
          {{
            "id": "dlg/greeting",
            "text": "Halt, traveler. This keep is mine to guard, and the door stays shut.",
            "options": [
              {{ "label": "Who are you?"{tooltip}, "next": "dlg/lore" }},
              {{
                "label": "Open the door, please.",
                "effects": [ {{ "type": "complete-objective", "objective": "obj/talk" }} ]
              }}
            ]
          }},
          {{
            "id": "dlg/lore",
            "text": "I am the Keeper. I have watched this gate since the moor swallowed the old road.",
            "options": [ {{ "label": "Back.", "next": "dlg/greeting" }} ]
          }}
        ]
      }}
    ]
  }}
}}"#
    )
}

/// The wine beat's shape: a caption on the button, the whole spoken line in the
/// hover box — far longer than any button could hold.
const FULL_LINE: &str =
    "And who are you, to come knocking at a door that has stayed shut for thirty winters?";

fn raw_with_dialogue(dialogue: String) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: common::read_valid("quests.json"),
        dialogue,
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

fn with_tooltip(version: &str) -> RawCampaign {
    raw_with_dialogue(dialogue_with_tooltip(
        &format!(
            ", \"tooltip\": {}",
            serde_json::Value::String(FULL_LINE.into())
        ),
        version,
    ))
}

#[test]
fn an_option_tooltip_validates_clean_at_v08() {
    let d = check_campaign(&with_tooltip("0.19.0"));
    assert!(
        d.is_empty(),
        "an authored tooltip must validate clean: {d:#?}"
    );
}

/// A tooltip is read by the player exactly as the caption is, so it is inventoried
/// and translated exactly as the caption is — under its own key, beside the label's.
#[test]
fn an_option_tooltip_enters_the_l10n_inventory() {
    let c = parse_campaign(&with_tooltip("0.19.0")).expect("parses");
    let inv = l10n::inventory(&c);
    assert_eq!(
        inv.get("dlg.keeper.greeting.opt.0.tooltip")
            .map(String::as_str),
        Some(FULL_LINE),
        "the tooltip must be inventoried under the option's own key: {:?}",
        inv.keys()
            .filter(|k| k.starts_with("dlg."))
            .collect::<Vec<_>>()
    );
    // The label keeps its key untouched — a tooltip never moves a content key.
    assert_eq!(
        inv.get("dlg.keeper.greeting.opt.0.label")
            .map(String::as_str),
        Some("Who are you?")
    );
    assert_eq!(inv, l10n::inventory(&c), "inventory is deterministic");
    // The traversal that inventories is the traversal that applies (`each_string`),
    // so a translated tooltip actually reaches the campaign.
    let mut c2 = c.clone();
    l10n::localize(
        &mut c2,
        &[(
            "dlg.keeper.greeting.opt.0.tooltip".to_string(),
            "你是谁，来敲这扇三十年没开过的门？".to_string(),
        )]
        .into_iter()
        .collect(),
    );
    assert_eq!(
        c2.dialogue.content.dialogues[0].nodes[0].options[0]
            .tooltip
            .as_deref(),
        Some("你是谁，来敲这扇三十年没开过的门？")
    );
}

/// An option with no tooltip contributes no key — coverage validation must not
/// start demanding translations for a string nobody authored.
#[test]
fn an_absent_tooltip_contributes_no_key() {
    let c =
        parse_campaign(&raw_with_dialogue(dialogue_with_tooltip("", "0.19.0"))).expect("parses");
    assert!(
        l10n::inventory(&c).keys().all(|k| !k.ends_with(".tooltip")),
        "an unauthored tooltip must be absent from the inventory"
    );
}
