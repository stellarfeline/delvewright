//! DSL v0.6 cutscene surface: the camera-aim field (`look_at`) and the
//! multi-shot `shots` list. Both are additive fields on the v0.4 `cutscene`
//! verb, reserved (`DW0141`) under a pre-0.6 quests stage; the single-shot
//! spelling stays valid forever. A cutscene that mixes the two spellings,
//! declares neither, or gives a shot with no camera waypoint is `DW0199`.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A quests document whose exit beat plays `cutscene`, with the effect body
/// spliced in — the one thing each case varies.
fn quests(version: &str, cutscene: &str) -> String {
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
        "on_objective_complete": {{
          "obj/talk": [ {{ "type": "open-gate", "anchor": "anchor/door" }} ]
        }},
        "on_complete": [ {cutscene}, {{ "type": "campaign-complete" }} ]
      }}
    ]
  }}
}}"#
    )
}

const SINGLE: &str = r#"{ "type": "cutscene", "seconds": 2,
  "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] },
            { "anchor": "anchor/exit", "offset": [0, 2, -2] } ] }"#;

const SINGLE_LOOK_AT: &str = r#"{ "type": "cutscene", "seconds": 2,
  "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] },
            { "anchor": "anchor/exit", "offset": [0, 2, -2] } ],
  "look_at": { "anchor": "anchor/keeper-stand", "offset": [0, 2, 0] } }"#;

const MULTI: &str = r#"{ "type": "cutscene", "shots": [
    { "seconds": 2,
      "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] },
                { "anchor": "anchor/exit", "offset": [0, 2, -2] } ] },
    { "seconds": 3,
      "path": [ { "anchor": "anchor/keeper-stand", "offset": [0, 2, 1] } ],
      "look_at": { "anchor": "anchor/keeper-stand", "offset": [0, 1, 0] } } ] }"#;

fn campaign_with_quests(quests: &str) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests.to_string(),
        dialogue: common::read_valid("dialogue.json"),
    }
}

fn diags(version: &str, cutscene: &str) -> Vec<delvewright_dsl::Diagnostic> {
    check_campaign(&campaign_with_quests(&quests(version, cutscene)))
}

/// The pre-0.6 single-shot spelling still validates clean under 0.4.0 — the
/// v0.6 additions never invalidate an existing campaign.
#[test]
fn single_shot_cutscene_still_validates_under_0_4() {
    let d = diags("0.4.0", SINGLE);
    assert!(d.is_empty(), "single-shot cutscene must stay valid: {d:#?}");
}

/// `look_at` and `shots` validate clean under 0.6.0.
#[test]
fn look_at_and_shots_validate_under_0_6() {
    for body in [SINGLE_LOOK_AT, MULTI] {
        let d = diags("0.6.0", body);
        assert!(d.is_empty(), "expected clean v0.6 cutscene: {d:#?}");
    }
}

/// `look_at` under a pre-0.6 quests stage is reserved → `DW0141`.
#[test]
fn look_at_reserved_before_0_6() {
    let d = diags("0.5.0", SINGLE_LOOK_AT);
    assert!(
        d.iter().any(|x| x.code == "DW0141"),
        "cutscene look_at must be reserved under 0.5.0: {d:#?}"
    );
}

/// The multi-shot `shots` list under a pre-0.6 quests stage is reserved → `DW0141`.
#[test]
fn shots_reserved_before_0_6() {
    let d = diags("0.5.0", MULTI);
    assert!(
        d.iter().any(|x| x.code == "DW0141"),
        "cutscene shots must be reserved under 0.5.0: {d:#?}"
    );
}

/// Mixing the multi-shot list with the single-shot fields is `DW0199`.
#[test]
fn mixing_shot_spellings_is_dw0199() {
    let mixed = r#"{ "type": "cutscene", "seconds": 2,
      "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] } ],
      "shots": [ { "seconds": 2,
                   "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] } ] } ] }"#;
    let d = diags("0.6.0", mixed);
    assert!(
        d.iter().any(|x| x.code == "DW0199"),
        "mixed cutscene spellings must be DW0199: {d:#?}"
    );
}

/// A cutscene declaring no shot at all is `DW0199`.
#[test]
fn cutscene_without_any_shot_is_dw0199() {
    let d = diags("0.6.0", r#"{ "type": "cutscene" }"#);
    assert!(
        d.iter().any(|x| x.code == "DW0199"),
        "shotless cutscene must be DW0199: {d:#?}"
    );
}

/// A shot with an empty camera `path` has nothing to look through → `DW0199`.
#[test]
fn shot_with_empty_path_is_dw0199() {
    let d = diags(
        "0.6.0",
        r#"{ "type": "cutscene", "shots": [ { "seconds": 2, "path": [] } ] }"#,
    );
    assert!(
        d.iter().any(|x| x.code == "DW0199"),
        "empty-path shot must be DW0199: {d:#?}"
    );
}

/// A single-shot cutscene missing `seconds` is `DW0199` (the schema can no longer
/// require the field — the shape check is what keeps it mandatory).
#[test]
fn single_shot_without_seconds_is_dw0199() {
    let d = diags(
        "0.6.0",
        r#"{ "type": "cutscene", "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] } ] }"#,
    );
    assert!(
        d.iter().any(|x| x.code == "DW0199"),
        "single-shot cutscene without seconds must be DW0199: {d:#?}"
    );
}

/// An unknown field inside a shot is a schema rejection (`deny_unknown_fields`).
#[test]
fn unknown_shot_field_is_dw0100() {
    let d = diags(
        "0.6.0",
        r#"{ "type": "cutscene", "shots": [ { "seconds": 2, "fov": 70,
             "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] } ] } ] }"#,
    );
    assert!(
        d.iter().any(|x| x.code == "DW0100"),
        "unknown shot field must be a schema rejection: {d:#?}"
    );
}
