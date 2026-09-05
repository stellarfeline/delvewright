//! The horizon library's two authoring refusals (spec-0026), and the promise
//! that keeps every campaign ever written compiling.
//!
//! * `DW0853` — a param out of its range, or a param sitting beside a base that
//!   reads nothing from it. The second half is the one that needs a test: the
//!   wire shape is flat, so `rim_height` on an `ocean` parses perfectly, and an
//!   author who wrote it believes something is reading it.
//! * `DW0855` — a base that BUILDS terrain, on a campaign that never says how
//!   big its map is. A surround rings a declared extent; `areas[]` states none.
//! * and the fence: the object form is `DW0141` below 0.16.0, while the two
//!   string shorthands stay writable at the version that introduced them, which
//!   is what makes this a widening rather than a break.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// A stage-1 world document at `version` whose `horizon` is the literal
/// `horizon` JSON — one area placed by `areas[]`, so the campaign states no
/// extent of its own unless a test gives it one.
fn world(version: &str, horizon: &str) -> String {
    format!(
        r#"{{
  "campaign_id": "hello-world",
  "content": {{
    "areas": [
      {{ "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }}
    ],
    "boundary": {{ "margin": 16 }},
    "horizon": {horizon},
    "premise": "One locked door stands between you and the road home. The Keeper holds the key, and only conversation will move him.",
    "seed": 20260729,
    "target_minutes": 5,
    "theme": "A lonely keep at the edge of the moor.",
    "title": "The Keeper's Door"
  }},
  "dsl_version": "{version}",
  "stage": "world"
}}"#
    )
}

fn codes(version: &str, horizon: &str) -> Vec<String> {
    let raw = RawCampaign {
        world: world(version, horizon),
        ..common::valid_raw()
    };
    check_campaign(&raw)
        .into_iter()
        .map(|d| d.code)
        .collect::<Vec<_>>()
}

/// **A surround needs a map to stand around, and `areas[]` is not one.**
///
/// The union of whatever `areas[]` places looks like an extent and is not one —
/// it is an artifact of the compiler's fixed area stride, mostly the void
/// between areas. Refused at validation rather than at the build, because it is
/// a fact about the documents: nothing has to be placed to know that nothing
/// states an extent.
#[test]
fn a_terrain_base_without_a_declared_region_is_dw0855() {
    let c = codes("0.19.0", r#"{ "base": "valley" }"#);
    assert!(c.contains(&"DW0855".to_string()), "codes: {c:?}");
}

/// The complement, and it is what keeps `DW0855` from being a refusal of the
/// horizon surface generally: the two bases that declare a world GENERATOR
/// rather than building one need no map at all, and neither raises it.
#[test]
fn a_generator_base_needs_no_map() {
    for horizon in [r#""void""#, r#""ocean""#, r#"{ "base": "ocean" }"#] {
        let c = codes("0.19.0", horizon);
        assert!(
            !c.contains(&"DW0855".to_string()),
            "{horizon} must not need a region; codes: {c:?}"
        );
    }
}

/// **A param out of range**, on the resolved view, with both bounds and the
/// default in the message.
#[test]
fn a_param_out_of_range_is_dw0853() {
    for horizon in [
        r#"{ "base": "valley", "ratio": 9.0 }"#,
        r#"{ "base": "valley", "ratio": 1.0 }"#,
        r#"{ "base": "valley", "rim_height": 4 }"#,
        r#"{ "base": "valley", "rim_height": 512 }"#,
    ] {
        let c = codes("0.19.0", horizon);
        assert!(
            c.contains(&"DW0853".to_string()),
            "{horizon} must be out of range; codes: {c:?}"
        );
    }
}

/// **A param beside a base that reads nothing from it.** The half that only
/// exists because the wire shape is flat, and the half a reader is most likely
/// to think is harmless: nothing about this document is malformed, and the
/// author is nonetheless wrong about what the engine is doing.
#[test]
fn a_param_foreign_to_its_base_is_dw0853() {
    for horizon in [
        r#"{ "base": "ocean", "rim_height": 40 }"#,
        r#"{ "base": "void", "ratio": 2.5 }"#,
    ] {
        let c = codes("0.19.0", horizon);
        assert!(
            c.contains(&"DW0853".to_string()),
            "{horizon} must be foreign; codes: {c:?}"
        );
    }
}

/// Inside the range, nothing fires. A check that reddened its own defaults
/// would be unfalsifiable in the useful direction.
#[test]
fn params_inside_their_range_are_accepted() {
    let c = codes(
        "0.19.0",
        r#"{ "base": "valley", "ratio": 2.5, "rim_height": 48 }"#,
    );
    assert!(!c.contains(&"DW0853".to_string()), "codes: {c:?}");
}

/// A body can walk out onto a valley's gap floor exactly as it can swim out
/// into the sea, so the return rule the sea has always needed is the gap
/// floor's too. `DW0320` asked whether the horizon was `ocean`; a rule keyed to
/// the base that happened to be first is a rule the second base escapes.
#[test]
fn a_horizon_a_body_can_enter_needs_a_boundary() {
    let no_boundary = world("0.19.0", r#"{ "base": "valley" }"#).replace(
        r#"    "boundary": { "margin": 16 },
"#,
        "",
    );
    assert!(!no_boundary.contains("boundary"), "the strip must have hit");
    let raw = RawCampaign {
        world: no_boundary,
        ..common::valid_raw()
    };
    let c: Vec<String> = check_campaign(&raw).into_iter().map(|d| d.code).collect();
    assert!(c.contains(&"DW0320".to_string()), "codes: {c:?}");
}
