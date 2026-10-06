//! spec-0082 acceptance criterion 1 — the assembly surface as the schema
//! export states it.
//!
//! `delvec schema --stage all` is `stage_schema` over every stage; the quests
//! stage is where the object class, its three verbs and its trigger kind live.

use delvewright_dsl::envelope::Stage;
use delvewright_dsl::{DSL_VERSION, stage_schema};
use serde_json::Value;

fn quests() -> Value {
    stage_schema(Stage::Quests)
}

/// The properties a definition declares, in name order.
fn props(schema: &Value, def: &str) -> Vec<String> {
    let mut v: Vec<String> = schema["$defs"][def]["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("`{def}` declares no properties"))
        .keys()
        .cloned()
        .collect();
    v.sort();
    v
}

/// Every tag a tagged union's branches carry under `tag`.
fn tags(schema: &Value, def: &str, tag: &str) -> Vec<String> {
    schema["$defs"][def]["oneOf"]
        .as_array()
        .unwrap_or_else(|| panic!("`{def}` is not a tagged union"))
        .iter()
        .filter_map(|b| b["properties"][tag]["const"].as_str().map(str::to_string))
        .collect()
}

/// The branch of a tagged union carrying `value`.
fn branch<'a>(schema: &'a Value, def: &str, tag: &str, value: &str) -> &'a Value {
    schema["$defs"][def]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["properties"][tag]["const"] == value)
        .unwrap_or_else(|| panic!("`{def}` has no `{value}` branch"))
}

#[test]
fn the_quests_stage_declares_assemblies() {
    let s = quests();
    let content = s["properties"]["content"]["$ref"]
        .as_str()
        .map(|r| r.trim_start_matches("#/$defs/").to_string())
        .unwrap_or_else(|| "QuestsContent".to_string());
    assert!(
        props(&s, &content).contains(&"assemblies".to_string()),
        "{:?}",
        props(&s, &content)
    );
    assert_eq!(
        props(&s, "Assembly"),
        ["at", "facing", "hitbox", "id", "initial", "rig", "strikes"]
    );
    assert_eq!(props(&s, "AssemblyHitbox"), ["height", "offset", "width"]);
    assert_eq!(props(&s, "AssemblyStrikes"), ["aim", "pattern", "while_in"]);
    assert_eq!(props(&s, "StrikeAim"), ["facings"]);
    assert_eq!(
        props(&s, "StrikeStep"),
        ["hold", "on_land", "strike", "ticks_per_frame", "windup"]
    );
    // `at` is a Mark, `while_in` the anchor-centred box `StealthZone` is.
    let at = &s["$defs"]["Assembly"]["properties"]["at"];
    assert!(
        at.to_string().contains("#/$defs/Mark"),
        "`at` is a Mark: {at}"
    );
    let while_in = &s["$defs"]["AssemblyStrikes"]["properties"]["while_in"];
    assert!(
        while_in.to_string().contains("#/$defs/StealthZone"),
        "{while_in}"
    );
}

#[test]
fn the_effect_union_gains_three_verbs() {
    let s = quests();
    let verbs = tags(&s, "QuestEffect", "type");
    // The union's size is stated once, by `v29_firework`'s
    // `the_effect_union_names_forty_three_verbs`; this test owns only that the
    // three assembly verbs are in it.
    for v in ["spawn-assembly", "despawn-assembly", "play-clip"] {
        assert!(verbs.contains(&v.to_string()), "{v} missing: {verbs:?}");
    }
    let play = branch(&s, "QuestEffect", "type", "play-clip");
    let mut fields: Vec<&str> = play["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .filter(|k| *k != "type")
        .collect();
    fields.sort();
    assert!(fields.starts_with(&["assembly", "clip"]), "{fields:?}");
}

#[test]
fn the_trigger_union_gains_strike_assembly() {
    let s = quests();
    let kinds = tags(&s, "TriggerOn", "on");
    assert_eq!(
        kinds,
        [
            "strike",
            "use",
            "approach",
            "step",
            "strike-npc",
            "strike-assembly"
        ]
    );
    let b = branch(&s, "TriggerOn", "on", "strike-assembly");
    let mut fields: Vec<&str> = b["properties"]
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort();
    assert_eq!(fields, ["assembly", "on"]);
}

/// The surface is stated at the `dsl_version` the crate manifest states.
#[test]
fn at_the_manifest_version() {
    let manifest = include_str!("../Cargo.toml");
    let version = manifest
        .lines()
        .find_map(|l| l.strip_prefix("version = \""))
        .and_then(|v| v.strip_suffix('"'))
        .expect("the manifest states a version");
    assert_eq!(DSL_VERSION, version);
}
