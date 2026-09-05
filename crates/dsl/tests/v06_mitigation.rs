//! DSL v0.6 stage-1 area `mitigation` — the first-class darkness declaration that
//! replaced the `DW0210` display-name heuristic. Validates under `dsl_version
//! 0.6.0`, reserved (`DW0141`) earlier.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

fn world_doc(version: &str, mitigation: bool) -> String {
    let mit = if mitigation {
        ",\n        \"mitigation\": \"night-vision\""
    } else {
        ""
    };
    format!(
        r#"{{
  "dsl_version": "{version}",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {{
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "areas": [
      {{
        "id": "area/keep",
        "name": "The Keep",
        "prefab": "prefab/hello-room"{mit}
      }}
    ]
  }}
}}"#
    )
}

fn campaign(world: &str) -> RawCampaign {
    RawCampaign {
        world: world.to_string(),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: common::read_valid("quests.json"),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

/// `mitigation: "night-vision"` validates clean under `dsl_version 0.6.0`.
#[test]
fn mitigation_validates_clean_at_0_6() {
    let diags = check_campaign(&campaign(&world_doc("0.19.0", true)));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for a v0.6 area mitigation, got: {diags:#?}"
    );
}

/// An unknown `mitigation` value is a schema rejection (`DW0100`), not a silent
/// pass — the enum is closed.
#[test]
fn unknown_mitigation_value_is_dw0100() {
    let bad = world_doc("0.19.0", true).replace("night-vision", "renamed-potion");
    let diags = check_campaign(&campaign(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0100"),
        "an unknown mitigation value must be a schema error: {diags:#?}"
    );
}

/// Absent `mitigation` (the pre-0.6 shape) still validates — the field is additive.
#[test]
fn absent_mitigation_is_unchanged() {
    let diags = check_campaign(&campaign(&world_doc("0.19.0", false)));
    assert!(
        diags.is_empty(),
        "absent mitigation must validate: {diags:#?}"
    );
}
