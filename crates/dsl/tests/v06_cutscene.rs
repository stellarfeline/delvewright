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
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
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

// --------------------------------------------------------------------------
// Shot styles (spec-0015): DW0348 / DW0349 / subject refs
// --------------------------------------------------------------------------

/// A well-formed styled shot — orbit around an anchor subject — validates
/// clean under 0.6.0 with no `path`/`seconds` (the style supplies both).
#[test]
fn styled_shot_validates_clean() {
    let d = diags(
        "0.6.0",
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "orbit-arc", "degrees": 90, "dist": 10,
               "subject": { "anchor": "anchor/exit", "offset": [0, 1, 0] } } ] }"#,
    );
    assert!(d.is_empty(), "styled shot must validate clean: {d:#?}");
}

/// Style-shape violations are `DW0348`: a style without `subject`, style params
/// on an unstyled shot, `subject_b` off `two-shot`, a `two-shot` without
/// `subject_b`, and `degrees` off `orbit-arc` / out of range.
#[test]
fn style_shape_violations_are_dw0348() {
    let cases = [
        // style, no subject
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "insert" } ] }"#,
        // style params without a style
        r#"{ "type": "cutscene", "shots": [ { "seconds": 2, "dist": 8,
             "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] } ] } ] }"#,
        // subject_b on a non-two-shot style
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "insert",
             "subject": { "anchor": "anchor/exit" },
             "subject_b": { "anchor": "anchor/door" } } ] }"#,
        // two-shot without subject_b
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "two-shot",
             "subject": { "anchor": "anchor/exit" } } ] }"#,
        // degrees off orbit-arc
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "insert", "degrees": 90,
             "subject": { "anchor": "anchor/exit" } } ] }"#,
        // orbit sweep out of the dossier's 45..=120 range
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "orbit-arc", "degrees": 361,
             "subject": { "anchor": "anchor/exit" } } ] }"#,
    ];
    for body in cases {
        let d = diags("0.6.0", body);
        assert!(
            d.iter().any(|x| x.code == "DW0348"),
            "expected DW0348 for {body}: {d:#?}"
        );
    }
}

/// `side-track`/`low-follow` need a subject that provably moves: an `anchor`
/// subject, or an npc with no sibling `move-npc`, is `DW0349`.
#[test]
fn follow_styles_without_motion_are_dw0349() {
    for body in [
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "side-track",
             "subject": { "anchor": "anchor/exit" } } ] }"#,
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "low-follow",
             "subject": { "npc": "npc/keeper" } } ] }"#,
    ] {
        let d = diags("0.6.0", body);
        assert!(
            d.iter().any(|x| x.code == "DW0349"),
            "expected DW0349 for {body}: {d:#?}"
        );
    }
}

/// The same follow shot with the subject's `move-npc` in the same effect list
/// is clean — the sibling move is the compiler-known motion.
#[test]
fn follow_style_with_sibling_move_is_clean() {
    let d = diags(
        "0.6.0",
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "side-track",
             "subject": { "npc": "npc/keeper" } } ] },
           { "type": "move-npc", "npc": "npc/keeper", "to_anchor": "anchor/exit" }"#,
    );
    assert!(
        !d.iter().any(|x| x.code == "DW0349"),
        "a sibling move-npc satisfies the moving-subject rule: {d:#?}"
    );
}

/// A subject naming an unknown npc is the standard dangling ref (`DW0112`).
#[test]
fn unknown_subject_npc_is_dangling_ref() {
    let d = diags(
        "0.6.0",
        r#"{ "type": "cutscene", "shots": [ { "shot_style": "insert",
             "subject": { "npc": "npc/nobody" } } ] }"#,
    );
    assert!(
        d.iter().any(|x| x.code == "DW0112"),
        "unknown subject npc must be DW0112: {d:#?}"
    );
}

/// `CameraShot`'s hand-written `Debug` is a stable content-key rendering: a
/// pre-style shot renders **byte-identically** to the pre-style derived struct
/// (the compiler's `sequence_key` hashes `{steps:?}`, so any drift would churn
/// every `seq_<hash>` function name in shipped campaigns).
#[test]
fn pre_style_shot_debug_rendering_is_stable() {
    let shot: delvewright_dsl::CameraShot = serde_json::from_str(
        r#"{ "seconds": 4,
             "path": [ { "anchor": "anchor/exit", "offset": [0, 2, 0] } ] }"#,
    )
    .unwrap();
    assert_eq!(
        format!("{shot:?}"),
        "CameraShot { path: [CameraWaypoint { anchor: AnchorId(\"anchor/exit\"), \
         offset: [0, 2, 0] }], seconds: 4, look_at: None }"
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

// --------------------------------------------------------------------------
// Shot subjects deny unknown fields
// --------------------------------------------------------------------------

/// A mistyped or over-specified shot subject must FAIL the schema (`DW0100`),
/// not be silently accepted with the stray key dropped.
///
/// `CameraSubject` is an untagged enum — the discriminator is the key name
/// (`anchor` / `npc` / `actor`). Serde has no variant-level
/// `deny_unknown_fields`, so with inline struct variants it quietly ignored any
/// key it did not recognise: a typo'd `ofset` deserialized fine with the offset
/// dropped, and a subject naming BOTH an anchor and an npc matched `Anchor` and
/// discarded the npc — a shot silently framing something the author never asked
/// for. Each variant now has its own `deny_unknown_fields` payload type.
#[test]
fn mistyped_shot_subject_fields_are_dw0100() {
    let cases = [
        // a typo'd `offset`
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "insert",
               "subject": { "anchor": "anchor/exit", "ofset": [0, 1, 0] } } ] }"#,
        // two discriminators at once — which one wins must never be implicit
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "insert",
               "subject": { "anchor": "anchor/exit", "npc": "npc/keeper" } } ] }"#,
        // an unknown key on an npc subject
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "insert",
               "subject": { "npc": "npc/keeper", "height": 2 } } ] }"#,
        // …and on `subject_b`
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "two-shot",
               "subject": { "anchor": "anchor/exit" },
               "subject_b": { "anchor": "anchor/door", "ofsett": [0, 1, 0] } } ] }"#,
    ];
    for body in cases {
        let d = diags("0.6.0", body);
        assert!(
            d.iter().any(|x| x.code == "DW0100"),
            "a mistyped shot subject must be a schema error, not silently \
             ignored — expected DW0100 for {body}: {d:#?}"
        );
    }
}

/// The control: the well-formed spellings of all three subject kinds still
/// deserialize, so the deny-unknown tightening did not narrow the surface.
#[test]
fn well_formed_shot_subjects_still_validate() {
    let d = diags(
        "0.6.0",
        r#"{ "type": "cutscene", "shots": [
             { "shot_style": "two-shot",
               "subject": { "anchor": "anchor/exit", "offset": [0, 1, 0] },
               "subject_b": { "npc": "npc/keeper" } } ] }"#,
    );
    assert!(d.is_empty(), "valid subjects must stay valid: {d:#?}");
}
