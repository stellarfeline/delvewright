//! DSL v0.5 (spec-0010): declared world `time`/`weather` validate, and a
//! place's `lighting` (its site-plan box's, spec-0106 §3.2) has its
//! `min_light` range-checked (`DW0196`).

mod common;

use delvewright_dsl::RawCampaign;
use std::sync::LazyLock;

/// A v0.5 stage-1 world document declaring time and weather.
static WORLD_V05: LazyLock<String> = LazyLock::new(|| {
    common::at_dsl_version(
        r#"{
  "dsl_version": "%dsl_version%",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "time": "midnight",
    "weather": "thunder"
  }
}"#,
    )
});

fn campaign_with_world(world: &str) -> RawCampaign {
    campaign_lit(world, 9)
}

fn campaign_lit(world: &str, min_light: u8) -> RawCampaign {
    let plan = common::patch_doc(&common::read_valid("site-plan.json"), |v| {
        v["content"]["boxes"][0]["lighting"] =
            serde_json::json!({ "fixture": "torch", "min_light": min_light });
    });
    RawCampaign {
        world: world.to_string(),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: common::read_valid("quests.json"),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: Some(common::read_valid("geometry-brief.json")),
        layout_graph: Some(common::read_valid("layout-graph.json")),
        site_plan: Some(plan),
        detail_plan: Some(common::read_valid("detail-plan.json")),
        design: None,
    }
}

/// v0.5 time/weather/lighting validate clean under `dsl_version 0.5.0`.
#[test]
fn v05_world_surface_validates_clean() {
    let diags = common::check_campaign(&campaign_with_world(WORLD_V05.as_str()));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.5 world surface, got: {diags:#?}"
    );
}

/// `lighting.min_light` outside 1..=14 is `DW0196`.
#[test]
fn v05_min_light_out_of_range_is_dw0196() {
    let diags = common::check_campaign(&campaign_lit(WORLD_V05.as_str(), 15));
    assert!(
        diags.iter().any(|d| d.code == "DW0196"),
        "min_light 15 must be DW0196 (range 1..=14): {diags:#?}"
    );
}
