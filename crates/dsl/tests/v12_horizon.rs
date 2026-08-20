//! DSL v0.12 (spec-0026 horizon library): the stage-1 `horizon` object form
//! `{base, …params}` and the new base/shorthand names.
//!
//! - The v0.6 strings (`"void"`/`"ocean"`) stay valid and byte-identical.
//! - The new surface is reserved (`DW0141`) below 0.12.0.
//! - `valley`/`summit`/`sky` (and `cherry-valley`) parse at 0.12.0 but are
//!   reserved-not-implemented in this engine slice (`DW0141`, the npc
//!   `vendor`/`boss` precedent) — the surround-generator slices delete that.
//! - `DW0366`: params out of range, and params foreign to the declared base.
//! - `DW0320` generalizes: every non-void base requires a `boundary`.
//! - `"cherry-valley"` desugars exactly to `{base:"valley", flora:"cherry",
//!   palette:"stone-petal"}` (acceptance criterion 6, DSL half).

mod common;

use delvewright_dsl::{
    Horizon, HorizonBase, HorizonFlora, HorizonPalette, RawCampaign, check_campaign,
};

/// A v0.12 stage-1 world document with a pluggable `horizon` value and an
/// always-on boundary.
fn world_doc(version: &str, horizon_json: &str, boundary: bool) -> String {
    let boundary_line = if boundary {
        "\"boundary\": { \"margin\": 24 },\n    "
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
    "horizon": {horizon_json},
    {boundary_line}"areas": [
      {{ "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }}
    ]
  }}
}}"#
    )
}

fn campaign_with_world(world: String) -> RawCampaign {
    RawCampaign {
        world,
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: common::read_valid("quests.json"),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
    }
}

fn diags_for(
    version: &str,
    horizon_json: &str,
    boundary: bool,
) -> Vec<delvewright_dsl::Diagnostic> {
    check_campaign(&campaign_with_world(world_doc(
        version,
        horizon_json,
        boundary,
    )))
}

/// The v0.6 string shorthands stay valid at 0.6.0 — no new obligation.
#[test]
fn ocean_string_still_validates_at_0_6() {
    let diags = diags_for("0.6.0", "\"ocean\"", true);
    assert!(diags.is_empty(), "{diags:#?}");
}

/// The object form — even for a base 0.6 already ships — is v0.12 surface:
/// `DW0141` below it.
#[test]
fn object_form_is_reserved_below_0_9() {
    let diags = diags_for("0.8.0", "{ \"base\": \"ocean\" }", true);
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "object form must be DW0141 under 0.8.0: {diags:#?}"
    );
}

/// A new shorthand name below 0.12.0 is `DW0141` too.
#[test]
fn flatland_string_is_reserved_below_0_9() {
    let diags = diags_for("0.8.0", "\"flatland\"", true);
    assert!(
        diags.iter().any(|d| d.code == "DW0141"),
        "flatland must be DW0141 under 0.8.0: {diags:#?}"
    );
}

/// `flatland` (with a boundary) is the landed new base: clean at 0.12.0, both
/// as a string and as the object form with its param.
#[test]
fn flatland_validates_clean_at_0_9() {
    for h in [
        "\"flatland\"",
        "{ \"base\": \"flatland\", \"blend_width\": 8 }",
    ] {
        let diags = diags_for("0.12.0", h, true);
        assert!(diags.is_empty(), "{h}: {diags:#?}");
    }
}

/// `DW0320` generalizes (spec-0026 §5): a flatland with no `boundary` is an
/// infinite walkable plain with no return rule.
#[test]
fn flatland_without_boundary_is_dw0320() {
    let diags = diags_for("0.12.0", "\"flatland\"", false);
    assert!(
        diags.iter().any(|d| d.code == "DW0320"),
        "flatland without boundary must be DW0320: {diags:#?}"
    );
}

/// `valley`/`summit`/`sky` parse at 0.12.0 but their surround generators have
/// not landed in this slice: reserved (`DW0141`), never silently mis-emitted.
#[test]
fn unlanded_bases_are_reserved_at_0_9() {
    for h in [
        "\"valley\"",
        "\"cherry-valley\"",
        "\"summit\"",
        "\"sky\"",
        "{ \"base\": \"valley\" }",
        "{ \"base\": \"summit\" }",
        "{ \"base\": \"sky\" }",
    ] {
        let diags = diags_for("0.12.0", h, true);
        assert!(
            diags.iter().any(|d| d.code == "DW0141"),
            "{h} must be reserved-not-implemented (DW0141): {diags:#?}"
        );
    }
}

/// `DW0366`: params out of their spec-0026 ranges.
#[test]
fn out_of_range_params_are_dw0366() {
    for h in [
        // valley ratio outside 2..=3
        "{ \"base\": \"valley\", \"ratio\": 3.5 }",
        "{ \"base\": \"valley\", \"ratio\": 1.0 }",
        // summit min_drop below the 100 floor
        "{ \"base\": \"summit\", \"min_drop\": 80 }",
        // summit plateau overflowing the build range after the gorge drop
        "{ \"base\": \"summit\", \"plateau_y\": 400 }",
        // sky walk plane outside the build range
        "{ \"base\": \"sky\", \"float_y\": 400 }",
    ] {
        let diags = diags_for("0.12.0", h, true);
        assert!(
            diags.iter().any(|d| d.code == "DW0366"),
            "{h} must be DW0366: {diags:#?}"
        );
    }
}

/// `DW0366`: a param declared on a base it does not belong to.
#[test]
fn foreign_params_are_dw0366() {
    for h in [
        "{ \"base\": \"ocean\", \"blend_width\": 4 }",
        "{ \"base\": \"flatland\", \"ratio\": 2.5 }",
        "{ \"base\": \"valley\", \"float_y\": 160 }",
    ] {
        let diags = diags_for("0.12.0", h, true);
        assert!(
            diags.iter().any(|d| d.code == "DW0366"),
            "{h} must be DW0366 (foreign param): {diags:#?}"
        );
    }
}

/// `"cherry-valley"` is a parameter row, not a base (spec-0026 acceptance
/// criterion 6, DSL half): it resolves to exactly the same view as
/// `{base:"valley", flora:"cherry", palette:"stone-petal"}`.
#[test]
fn cherry_valley_desugars_to_valley_params() {
    let shorthand: Horizon = serde_json::from_str("\"cherry-valley\"").unwrap();
    let object: Horizon = serde_json::from_str(
        "{ \"base\": \"valley\", \"flora\": \"cherry\", \"palette\": \"stone-petal\" }",
    )
    .unwrap();
    assert_eq!(shorthand.resolved(), object.resolved());
    let r = shorthand.resolved();
    assert_eq!(r.base, HorizonBase::Valley);
    assert_eq!(r.flora, HorizonFlora::Cherry);
    assert_eq!(r.palette, HorizonPalette::StonePetal);
    // …and the plain valley differs ONLY in those two fields.
    let plain: Horizon = serde_json::from_str("\"valley\"").unwrap();
    let p = plain.resolved();
    assert_eq!(
        delvewright_dsl::ResolvedHorizon {
            flora: HorizonFlora::Cherry,
            palette: HorizonPalette::StonePetal,
            ..p
        },
        r
    );
}

/// Both wire forms round-trip through serde unchanged — the string shorthand
/// stays a string (the byte-identity half of the fence: a v0.6 document
/// re-serialized carries the same `"ocean"` it declared).
#[test]
fn wire_forms_round_trip() {
    for (json, want) in [
        ("\"ocean\"", "\"ocean\""),
        ("\"cherry-valley\"", "\"cherry-valley\""),
        (
            "{\"base\":\"flatland\",\"blend_width\":8}",
            "{\"base\":\"flatland\",\"blend_width\":8}",
        ),
    ] {
        let h: Horizon = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&h).unwrap(), want);
    }
}
