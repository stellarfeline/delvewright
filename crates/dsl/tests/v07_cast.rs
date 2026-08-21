//! DSL v0.7 (spec-0020): the per-quest `cast` ledger — version gating, schema
//! export, dialogue reachability through ledger roots, and bark l10n.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign, l10n, parse_campaign};

/// hello-world's single quest, plus a cast ledger. `dsl_version` is a parameter
/// so the same document can be tested on both sides of the 0.7 boundary.
fn quests(version: &str) -> String {
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
          {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }}
        ],
        "on_complete": [ {{ "type": "campaign-complete" }} ],
        "cast": {{
          "npc/keeper": {{
            "at": "anchor/keeper-stand",
            "doing": "barring the door with his body",
            "dialogue": "dlg/greeting"
          }}
        }}
      }}
    ]
  }}
}}"#
    )
}

fn campaign_with(quests: String) -> RawCampaign {
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
    }
}

/// A `cast` ledger validates clean under `dsl_version 0.7.0`.
#[test]
fn cast_validates_clean_under_v07() {
    let diags = check_campaign(&campaign_with(quests("0.7.0")));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.7 cast ledger, got: {diags:#?}"
    );
}

/// A `cast` ledger in a pre-0.7 campaign is a reserved construct — `DW0141`.
/// (The *absence* of a ledger is what the deprecation window forgives; declaring
/// one below its version is not.)
#[test]
fn cast_is_reserved_before_v07() {
    let diags = check_campaign(&campaign_with(quests("0.6.0")));
    let d = diags
        .iter()
        .find(|d| d.code == "DW0141")
        .unwrap_or_else(|| panic!("expected DW0141, got: {diags:#?}"));
    assert!(d.path.ends_with("/cast"), "{}", d.path);
    assert!(d.message.contains("0.7.0"), "{}", d.message);
}

/// A node reached only as a ledger root is reachable, not orphaned: swapping to a
/// later root must not make that root `DW0120`.
#[test]
fn a_ledger_root_is_a_dialogue_entry_point() {
    // Add an extra node nothing links to, and cast it as a later root.
    let dialogue = common::read_valid("dialogue.json").replace(
        r#""nodes": ["#,
        r#""nodes": [
          { "id": "dlg/farewell", "text": "The road is yours.", "options": [] },"#,
    );
    let with_root = quests("0.7.0").replace(
        r#""dialogue": "dlg/greeting""#,
        r#""dialogue": "dlg/farewell""#,
    );
    let mut raw = campaign_with(with_root);
    raw.dialogue = dialogue.clone();
    let diags = check_campaign(&raw);
    assert!(
        !diags.iter().any(|d| d.code == "DW0120"),
        "a cast-declared root must count as an entry point, got: {diags:#?}"
    );

    // Control: the same orphan node with NO ledger reference is still DW0120.
    let mut orphan = campaign_with(quests("0.7.0"));
    orphan.dialogue = dialogue;
    assert!(
        check_campaign(&orphan).iter().any(|d| d.code == "DW0120"),
        "an genuinely unreferenced node must still be unreachable"
    );
}

/// Bark lines are player-visible, so they enter the l10n inventory (keyed to the
/// speaking NPC); `doing` is authoring context and deliberately stays out.
#[test]
fn bark_lines_enter_the_l10n_inventory() {
    let barks = quests("0.7.0").replace(
        r#""dialogue": "dlg/greeting""#,
        r#""dialogue": { "barks": ["Mind the step.", "Cold tonight."] }"#,
    );
    let c = parse_campaign(&campaign_with(barks)).expect("parses");
    let inv = l10n::inventory(&c);
    let keys: Vec<&String> = inv
        .keys()
        .filter(|k| k.starts_with("cast."))
        .collect::<Vec<_>>();
    assert_eq!(
        keys.len(),
        2,
        "both bark lines must be inventoried, got: {keys:?}"
    );
    let key = keys[0];
    assert_eq!(inv[key], "Mind the step.");
    assert_eq!(
        l10n::key_speaker(key),
        Some("keeper"),
        "a bark must resolve to its speaking NPC so a translator gets the persona"
    );
    assert!(
        !inv.values().any(|v| v.contains("barring the door")),
        "`doing` is authoring context, never shown to a player"
    );
}

/// The stage-5 schema export carries `cast` (the skill generates against it).
#[test]
fn stage5_schema_exports_cast() {
    let schema = delvewright_dsl::schema::stage_schema(delvewright_dsl::Stage::Quests);
    let json = serde_json::to_string(&schema).unwrap();
    assert!(
        json.contains("\"cast\""),
        "stage-5 schema must carry `cast`"
    );
    assert!(
        json.contains("barks"),
        "stage-5 schema must carry the bark-pool form"
    );
    assert!(
        json.contains("unchanged"),
        "stage-5 schema must carry the `unchanged` keyword"
    );
}
