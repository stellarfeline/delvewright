//! DSL v0.5 (spec-0010): declared world `time`/`weather` + per-area `lighting`
//! and the `set-time`/`set-weather` effect verbs validate under `0.5.0` and are
//! reserved (`DW0141`) earlier; `lighting.min_light` is range-checked (`DW0196`).
//!
//! Built on the hello-world casting/quests/dialogue (unchanged, 0.2.0) with a
//! v0.5 stage-1 `world` document — additive fields, so the rest is untouched.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.5 stage-1 world document declaring time, weather and an area `lighting`.
const WORLD_V05: &str = r#"{
  "dsl_version": "0.5.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "time": "midnight",
    "weather": "thunder",
    "areas": [
      {
        "id": "area/keep",
        "name": "The Keep",
        "prefab": "prefab/hello-room",
        "lighting": { "fixture": "lantern", "min_light": 9 }
      }
    ]
  }
}"#;

fn campaign_with_world(world: &str) -> RawCampaign {
    RawCampaign {
        world: world.to_string(),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: common::read_valid("quests.json"),
        dialogue: common::read_valid("dialogue.json"),
    }
}

/// v0.5 time/weather/lighting validate clean under `dsl_version 0.5.0`.
#[test]
fn v05_world_surface_validates_clean() {
    let diags = check_campaign(&campaign_with_world(WORLD_V05));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.5 world surface, got: {diags:#?}"
    );
}

/// The same fields under a pre-0.5 world version are reserved → `DW0141`.
#[test]
fn v05_world_surface_reserved_before_0_5() {
    let pre = WORLD_V05.replacen("\"0.5.0\"", "\"0.4.0\"", 1);
    let diags = check_campaign(&campaign_with_world(&pre));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "v0.5 world surface must be reserved under 0.4.0 (DW0141): {diags:#?}"
    );
}

/// `lighting.min_light` outside 1..=14 is `DW0196`.
#[test]
fn v05_min_light_out_of_range_is_dw0196() {
    let bad = WORLD_V05.replacen("\"min_light\": 9", "\"min_light\": 15", 1);
    let diags = check_campaign(&campaign_with_world(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0196"),
        "min_light 15 must be DW0196 (range 1..=14): {diags:#?}"
    );
}
