//! DSL v0.10 lethal volumes (spec-0031): the version fence, the structural
//! checks, and the l10n obligation the death wording carries.

use std::collections::BTreeMap;

use delvewright_dsl::{RawCampaign, check_campaign, l10n_inventory, parse_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

/// A hello-world `quests` doc at `version` carrying `volumes` as its
/// `lethal_volumes` body (a raw JSON array body, no surrounding brackets).
fn quests_doc(version: &str, volumes: &str) -> String {
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
    "lethal_volumes": [ {volumes} ]
  }}
}}"#
    )
}

fn raw(quests: String) -> RawCampaign {
    RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests,
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
    }
}

/// The canonical well-formed volume: an anchor-centred box and a wording.
const GOOD: &str = r#"{
  "id": "lethal/the-drop",
  "region": { "anchor": "anchor/exit", "extent": [1, 2, 1] },
  "message": "The undertow takes you.",
  "damage_type": "fall"
}"#;

fn codes(quests: String) -> Vec<String> {
    check_campaign(&raw(quests))
        .into_iter()
        .map(|d| d.code.to_string())
        .collect()
}

/// A lethal volume declared at 0.10.0 validates clean.
#[test]
fn a_well_formed_volume_validates_clean() {
    let d = check_campaign(&raw(quests_doc("0.10.0", GOOD)));
    assert!(
        d.is_empty(),
        "a v0.10 lethal volume validates clean: {d:#?}"
    );
}

/// Declaring one below 0.10.0 is `DW0141` — the version ledger every surface
/// follows. This is the half that keeps every existing campaign compiling: the
/// field cannot appear in one.
#[test]
fn a_volume_below_v010_is_dw0141() {
    let c = codes(quests_doc("0.9.0", GOOD));
    assert!(
        c.iter().any(|x| x == "DW0141"),
        "a lethal volume at dsl_version 0.9.0 is reserved: {c:?}"
    );
}

/// A blank wording is `DW0512`: the volume would kill in silence, which is the
/// one thing the declaration exists to prevent.
#[test]
fn a_blank_message_is_dw0512() {
    let c = codes(quests_doc(
        "0.10.0",
        r#"{ "id": "lethal/mute", "region": { "anchor": "anchor/exit", "extent": [1, 1, 1] },
             "message": "   " }"#,
    ));
    assert!(
        c.iter().any(|x| x == "DW0512"),
        "a blank lethal-volume message is DW0512: {c:?}"
    );
}

/// A malformed id is `DW0110`, a repeated one `DW0111`, and an anchor no bound
/// prefab provides is `DW0142` — the same three every stage-5 declaration owes.
#[test]
fn id_and_anchor_defects_are_reported() {
    let c = codes(quests_doc(
        "0.10.0",
        r#"{ "id": "lethal/Bad Id", "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
             "message": "a" },
           { "id": "lethal/dup", "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
             "message": "a" },
           { "id": "lethal/dup", "region": { "anchor": "anchor/nowhere", "extent": [0, 0, 0] },
             "message": "a" }"#,
    ));
    for want in ["DW0110", "DW0111", "DW0142"] {
        assert!(c.iter().any(|x| x == want), "expected {want}: {c:?}");
    }
}

/// The wording enters the l10n inventory like every other player-visible string,
/// under a key derived from the volume's own id.
#[test]
fn the_message_is_inventoried() {
    let c = parse_campaign(&raw(quests_doc("0.10.0", GOOD))).expect("parses");
    let inv: BTreeMap<String, String> = l10n_inventory(&c);
    assert_eq!(
        inv.get("lethal.the-drop.message").map(String::as_str),
        Some("The undertow takes you."),
        "the death wording is inventoried under `lethal.<id>.message`: {inv:#?}"
    );
}

/// An unknown field is a schema rejection, not a silently ignored one — the
/// struct is `deny_unknown_fields` like every sibling declaration.
#[test]
fn an_unknown_field_is_a_schema_rejection() {
    let c = codes(quests_doc(
        "0.10.0",
        r#"{ "id": "lethal/x", "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
             "message": "a", "kills_players_only": true }"#,
    ));
    assert!(
        c.iter().any(|x| x == "DW0100"),
        "an unknown lethal-volume field is DW0100: {c:?}"
    );
}
