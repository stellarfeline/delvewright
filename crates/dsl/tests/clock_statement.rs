//! The clock statement (spec-0122 §7.1): `world.time` takes the tick within the
//! day, with the moon's phase where the moon shows, and emits through the one
//! `time set` token every other spelling goes through.

mod common;

use delvewright_dsl::{WorldTime, parse_campaign, validate_campaign};
use serde_json::{Value, json};

/// The hello-world fixture with `world.time` replaced, parsed and validated:
/// the parse error text when the document does not parse, else every
/// diagnostic code and the parsed time.
fn with_time(time: Value) -> Result<(WorldTime, Vec<String>), String> {
    let mut raw = common::valid_raw();
    raw.world = common::patch_doc(&raw.world, |w| w["content"]["time"] = time);
    let c = parse_campaign(&raw).map_err(|d| {
        d.iter()
            .map(|d| format!("{}: {}", d.code, d.message))
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    let codes = validate_campaign(&c)
        .into_iter()
        .map(|d| d.code.to_string())
        .collect();
    Ok((c.world.content.time, codes))
}

fn token(t: WorldTime) -> String {
    format!("time set {}", t.token(t.world_clock()))
}

#[test]
fn late_afternoon_is_a_tick_on_day_zero() {
    let (t, codes) = with_time(json!({"daytime": 10500})).expect("parses");
    assert!(!codes.contains(&"DW0931".to_string()), "{codes:?}");
    assert_eq!(token(t), "time set 10500");
    assert_eq!(t.keyword(), r#"{"daytime":10500}"#);
}

#[test]
fn a_visible_moon_owes_its_phase_and_the_phase_names_the_day() {
    let (_, codes) = with_time(json!({"daytime": 15000})).expect("parses");
    assert!(codes.contains(&"DW0931".to_string()), "{codes:?}");
    let (t, codes) =
        with_time(json!({"daytime": 15000, "phase": "new-moon"})).expect("parses");
    assert!(!codes.contains(&"DW0931".to_string()), "{codes:?}");
    assert_eq!(token(t), "time set 111000");
}

#[test]
fn a_phase_under_a_set_moon_is_refused() {
    let (_, codes) =
        with_time(json!({"daytime": 6000, "phase": "new-moon"})).expect("parses");
    assert!(codes.contains(&"DW0931".to_string()), "{codes:?}");
}

#[test]
fn a_tick_past_the_day_is_the_schemas_refusal() {
    let err = with_time(json!({"daytime": 24000})).expect_err("refused at parse");
    assert!(err.starts_with("DW0100"), "{err}");
    assert!(err.contains("0..=23999"), "{err}");
    let err = with_time(json!({"daytime": -1})).expect_err("refused at parse");
    assert!(err.starts_with("DW0100"), "{err}");
}

#[test]
fn the_schema_offers_three_spellings() {
    let schema = serde_json::to_value(schemars::schema_for!(WorldTime)).expect("schema");
    let any = schema["anyOf"].as_array().expect("anyOf");
    assert_eq!(any.len(), 3, "{schema:#}");
}

#[test]
fn canonical_form_is_byte_stable() {
    let t: WorldTime =
        serde_json::from_str(r#"{"phase":"new-moon","daytime":15000}"#).expect("parses");
    let s = delvewright_dsl::to_canonical_string(&t).expect("serialises");
    let again: WorldTime = serde_json::from_str(&s).expect("re-parses");
    assert_eq!(t, again);
    assert_eq!(
        delvewright_dsl::to_canonical_string(&again).expect("serialises"),
        s
    );
    assert!(s.find("daytime") < s.find("phase"), "{s}");
}
