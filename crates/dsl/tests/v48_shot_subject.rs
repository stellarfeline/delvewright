//! spec-0110 criteria 1 and 2: every cutscene shot names its subject.
//!
//! 1. The quests schema exports `subject` required on every shot in both
//!    cutscene spellings, with its seven kinds, and `seen_from` on
//!    `assemblies[]`; `DSL_VERSION` is 0.48.0.
//! 2. One fixture per shape: no subject is `DW0199` naming the shot;
//!    `"landscape"` under a style or as `subject_b` is `DW0348`; a subject on
//!    an unstyled shot is accepted; an unknown assembly or place is `DW0112`;
//!    two kinds at once is `DW0100`.

mod common;

use delvewright_dsl::{DSL_VERSION, Diagnostic, RawCampaign, Stage, check_campaign, stage_schema};
use serde_json::Value;

fn defs() -> Value {
    let s = serde_json::to_value(stage_schema(Stage::Quests)).unwrap();
    s["$defs"].clone()
}

#[test]
fn the_schema_requires_a_subject_on_every_shot_in_both_spellings() {
    let d = defs();
    let shot = &d["CameraShot"];
    assert!(
        shot["required"]
            .as_array()
            .is_some_and(|r| r.iter().any(|v| v == "subject")),
        "a `shots` entry requires `subject`: {shot}"
    );
    let cutscene = d["QuestEffect"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["properties"]["type"]["const"] == "cutscene")
        .expect("the cutscene variant");
    assert!(cutscene["properties"]["subject"].is_object());
    assert_eq!(
        cutscene["dependentRequired"],
        serde_json::json!({"path": ["subject"], "seconds": ["subject"]}),
        "the single-shot spelling requires `subject` beside `path`/`seconds`"
    );
}

#[test]
fn the_subject_has_seven_kinds() {
    let d = defs();
    let kinds = d["CameraSubject"]["anyOf"].as_array().unwrap();
    let refs: Vec<&str> = kinds
        .iter()
        .map(|k| k["$ref"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        refs,
        vec![
            "#/$defs/Mark",
            "#/$defs/NpcSubject",
            "#/$defs/ActorSubject",
            "#/$defs/AssemblySubject",
            "#/$defs/PlaceSubject",
            "#/$defs/RegionSubject",
            "#/$defs/Landscape",
        ]
    );
    assert_eq!(d["Landscape"]["oneOf"][0]["const"], "landscape");
}

#[test]
fn assemblies_export_seen_from_and_the_version_is_0_48_0() {
    let d = defs();
    assert!(d["Assembly"]["properties"]["seen_from"].is_object());
    assert_eq!(d["SeenFrom"]["minimum"], 1);
    assert_eq!(d["SeenFrom"]["maximum"], 512);
    assert_eq!(DSL_VERSION, "0.48.0");
}

// --- criterion 2: the shapes ------------------------------------------------

fn quests(cutscene: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{DSL_VERSION}",
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

fn diags(cutscene: &str) -> Vec<Diagnostic> {
    let raw = RawCampaign {
        quests: quests(cutscene),
        ..common::valid_raw()
    };
    check_campaign(&raw)
}

const PATH: &str = r#""path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] } ]"#;

#[test]
fn a_shot_with_no_subject_is_dw0199_naming_the_shot() {
    let d = diags(&format!(
        r#"{{ "type": "cutscene", "shots": [ {{ "seconds": 2, "subject": "landscape", {PATH} }},
                                          {{ "seconds": 2, {PATH} }} ] }}"#
    ));
    let hit = d
        .iter()
        .find(|x| x.code == "DW0199")
        .unwrap_or_else(|| panic!("DW0199 expected: {d:#?}"));
    assert!(hit.path.ends_with("/shots/1/subject"), "{hit:?}");
    assert!(hit.message.contains("shot 1"), "{hit:?}");
    // The single-shot spelling too.
    let d = diags(&format!(
        r#"{{ "type": "cutscene", "seconds": 2, {PATH} }}"#
    ));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0199" && x.path.ends_with("/subject")),
        "{d:#?}"
    );
}

#[test]
fn a_landscape_under_a_style_or_as_subject_b_is_dw0348() {
    for body in [
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "push-in", "subject": "landscape" } ] }"#,
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "two-shot",
             "subject": { "anchor": "anchor/exit" }, "subject_b": "landscape" } ] }"#,
    ] {
        let d = diags(body);
        assert!(d.iter().any(|x| x.code == "DW0348"), "{body}: {d:#?}");
    }
}

#[test]
fn a_subject_on_an_unstyled_shot_is_accepted() {
    for subject in [
        r#"{ "anchor": "anchor/exit" }"#,
        r#"{ "npc": "npc/keeper" }"#,
        r#""landscape""#,
        r#"{ "region": { "anchor": "anchor/exit", "extent": [1, 0, 1] } }"#,
    ] {
        let d = diags(&format!(
            r#"{{ "type": "cutscene", "seconds": 2, "subject": {subject}, {PATH} }}"#
        ));
        assert!(d.is_empty(), "{subject}: {d:#?}");
    }
}

#[test]
fn an_unknown_assembly_or_place_is_dw0112() {
    for subject in [
        r#"{ "assembly": "assembly/nothing" }"#,
        r#"{ "place": "node/nowhere" }"#,
    ] {
        let d = diags(&format!(
            r#"{{ "type": "cutscene", "shots": [ {{ "seconds": 2, "subject": {subject}, {PATH} }} ] }}"#
        ));
        assert!(
            d.iter()
                .any(|x| x.code == "DW0112" && x.path.ends_with("/shots/0/subject")),
            "{subject}: {d:#?}"
        );
    }
}

#[test]
fn a_subject_naming_two_kinds_is_dw0100() {
    let d = diags(&format!(
        r#"{{ "type": "cutscene", "seconds": 2,
             "subject": {{ "assembly": "assembly/x", "place": "area/hall" }}, {PATH} }}"#
    ));
    assert!(d.iter().any(|x| x.code == "DW0100"), "{d:#?}");
}

#[test]
fn a_seen_from_outside_1_to_512_is_refused_at_parse() {
    for v in [0, 513] {
        let r: Result<delvewright_dsl::SeenFrom, _> = serde_json::from_value(serde_json::json!(v));
        assert!(r.is_err(), "{v}");
    }
    let ok: delvewright_dsl::SeenFrom = serde_json::from_value(serde_json::json!(512)).unwrap();
    assert_eq!(ok.blocks(), 512);
}
