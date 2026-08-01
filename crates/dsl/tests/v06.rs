//! DSL v0.6 (spec-0013): the stage-1 `horizon` and `boundary` world fields
//! validate under `0.6.0` and are reserved (`DW0141`) earlier. Under 0.6.0,
//! `horizon: "ocean"` without a `boundary` is `DW0320` and a `boundary.margin`
//! outside `0..=64` is `DW0321`.
//!
//! Built on the hello-world casting/quests/dialogue (unchanged, 0.2.0) with a
//! v0.6 stage-1 `world` document — additive fields, so the rest is untouched.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A v0.6 stage-1 world document: ocean horizon + a boundary (the happy path).
const WORLD_V06: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "ocean",
    "boundary": { "margin": 24, "message": "The tide turns you back." },
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
    ]
  }
}"#;

/// Ocean horizon with NO boundary — the `DW0320` authoring error.
const WORLD_V06_OCEAN_NO_BOUNDARY: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "ocean",
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
    ]
  }
}"#;

/// Explicit void horizon, no boundary — valid (void needs no return rule).
const WORLD_V06_VOID: &str = r#"{
  "dsl_version": "0.6.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "void",
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
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

/// v0.6 horizon + boundary validate clean under `dsl_version 0.6.0`.
#[test]
fn v06_world_surface_validates_clean() {
    let diags = check_campaign(&campaign_with_world(WORLD_V06));
    assert!(
        diags.is_empty(),
        "expected zero diagnostics for the v0.6 world surface, got: {diags:#?}"
    );
}

/// The same fields under a pre-0.6 world version are reserved -> `DW0141`.
#[test]
fn v06_world_surface_reserved_before_0_6() {
    let pre = WORLD_V06.replacen("\"0.6.0\"", "\"0.5.0\"", 1);
    let diags = check_campaign(&campaign_with_world(&pre));
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "v0.6 world surface must be reserved under 0.5.0 (DW0141): {diags:#?}"
    );
}

/// `horizon: "ocean"` without a `boundary` is `DW0320`.
#[test]
fn v06_ocean_without_boundary_is_dw0320() {
    let diags = check_campaign(&campaign_with_world(WORLD_V06_OCEAN_NO_BOUNDARY));
    assert!(
        diags.iter().any(|d| d.code == "DW0320"),
        "ocean without boundary must be DW0320: {diags:#?}"
    );
}

/// `boundary.margin` above 64 is `DW0321`.
#[test]
fn v06_margin_out_of_range_is_dw0321() {
    let bad = WORLD_V06.replacen("\"margin\": 24", "\"margin\": 65", 1);
    let diags = check_campaign(&campaign_with_world(&bad));
    assert!(
        diags.iter().any(|d| d.code == "DW0321"),
        "margin 65 must be DW0321 (range 0..=64): {diags:#?}"
    );
}

/// `margin: 0` is in range (0..=64 inclusive) — no `DW0321`.
#[test]
fn v06_margin_zero_is_in_range() {
    let ok = WORLD_V06.replacen("\"margin\": 24", "\"margin\": 0", 1);
    let diags = check_campaign(&campaign_with_world(&ok));
    assert!(
        !diags.iter().any(|d| d.code == "DW0321"),
        "margin 0 is in range and must not be DW0321: {diags:#?}"
    );
}

/// An explicit `void` horizon with no boundary validates clean.
#[test]
fn v06_void_horizon_needs_no_boundary() {
    let diags = check_campaign(&campaign_with_world(WORLD_V06_VOID));
    assert!(
        diags.is_empty(),
        "explicit void horizon needs no boundary: {diags:#?}"
    );
}
