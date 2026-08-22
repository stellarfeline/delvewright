//! DSL v0.11 body `traversal` (spec-0034): the
//! author's side of the traversal proof.
//!
//! Spiders really do climb, so the traversal rules cannot be absolute — and what
//! was missing was a way for an author to say "this body is an exception" and
//! have the claim PROVEN, instead of the exception happening by accident and
//! merely rendering.
//!
//! This file covers the DSL half: the surface exists on **both** body classes,
//! it is fenced **per stage** (`DW0141`), and a value the compiler could never
//! hold a body to is refused at declaration time (`DW0455`) rather than accepted
//! and silently ignored. The proof half — a declaration must change a verdict
//! (`DW0454`) — lives in `delvec`'s `tests/traversal.rs`, where there is a world
//! to be held to.

mod common;

use delvewright_dsl::{RawCampaign, check_campaign};

/// hello-world's npcs stage, at `version`, with the keeper optionally declaring
/// a `traversal`.
fn npcs(version: &str, locomotion: Option<&str>) -> String {
    let mut doc: serde_json::Value =
        serde_json::from_str(&common::read_valid("npcs.json")).unwrap();
    doc["dsl_version"] = serde_json::json!(version);
    if let Some(l) = locomotion {
        doc["content"]["npcs"][0]["traversal"] = serde_json::json!({ "locomotion": l });
    }
    doc.to_string()
}

/// hello-world's quests stage, at `version`, with one actor optionally declaring
/// a `traversal`.
fn quests(version: &str, locomotion: Option<&str>) -> String {
    let mut doc: serde_json::Value =
        serde_json::from_str(&common::read_valid("quests.json")).unwrap();
    doc["dsl_version"] = serde_json::json!(version);
    let mut actor = serde_json::json!({
        "id": "actor/subject",
        "entity": "minecraft:sheep",
        "anchor": "anchor/keeper-stand"
    });
    if let Some(l) = locomotion {
        actor["traversal"] = serde_json::json!({ "locomotion": l });
    }
    doc["content"]["actors"] = serde_json::json!([actor]);
    doc.to_string()
}

fn raw(npcs_doc: String, quests_doc: String) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json"),
        npcs: npcs_doc,
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: quests_doc,
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    }
}

fn codes(raw: &RawCampaign) -> Vec<String> {
    check_campaign(raw)
        .iter()
        .map(|d| d.code.to_string())
        .collect()
}

/// The surface exists on the stage-2 NPC and the stage-5 actor, at 0.11.0, with
/// the same shape — one type, two consumers.
///
/// Generality is decided at the FIRST site (CLAUDE.md): a field built onto one
/// body class leaves the second with no surface, and the "fix" is then a second
/// bespoke field. So both are exercised here, not one.
#[test]
fn both_body_classes_accept_a_declaration_at_v11() {
    for locomotion in ["ground", "climber", "flier"] {
        let r = raw(npcs("0.11.0", Some(locomotion)), quests("0.11.0", None));
        assert!(
            !codes(&r).iter().any(|c| c == "DW0141" || c == "DW0455"),
            "npc `{locomotion}`: {:#?}",
            check_campaign(&r)
        );
        let r = raw(npcs("0.11.0", None), quests("0.11.0", Some(locomotion)));
        assert!(
            !codes(&r).iter().any(|c| c == "DW0141" || c == "DW0455"),
            "actor `{locomotion}`: {:#?}",
            check_campaign(&r)
        );
    }
}

/// …and the fence is **per stage**, which is the whole point of a per-stage
/// fence: an npcs document may adopt the surface while the quests document stays
/// where it was, and vice versa. Declaring it below 0.11.0 is `DW0141`.
#[test]
fn a_declaration_below_v11_is_dw0141_in_its_own_stage() {
    let r = raw(npcs("0.10.0", Some("climber")), quests("0.11.0", None));
    let d = check_campaign(&r);
    let hit = d
        .iter()
        .find(|d| d.code == "DW0141")
        .unwrap_or_else(|| panic!("expected DW0141 for the npcs stage: {d:#?}"));
    assert_eq!(hit.stage, "npcs");
    assert!(hit.path.contains("/content/npcs/0/traversal"), "{hit:?}");

    let r = raw(npcs("0.11.0", None), quests("0.10.0", Some("climber")));
    let d = check_campaign(&r);
    let hit = d
        .iter()
        .find(|d| d.code == "DW0141")
        .unwrap_or_else(|| panic!("expected DW0141 for the quests stage: {d:#?}"));
    assert_eq!(hit.stage, "quests");
    assert!(hit.path.contains("/content/actors/0/traversal"), "{hit:?}");

    // …and the stage that DID adopt it raises nothing, or the fence would be a
    // campaign-wide gate wearing a per-stage name.
    let r = raw(npcs("0.11.0", Some("climber")), quests("0.10.0", None));
    assert!(
        !codes(&r).iter().any(|c| c == "DW0141"),
        "{:#?}",
        check_campaign(&r)
    );
}

/// **A value the engine cannot hold a body to is refused, not ignored**
/// (`DW0455`). `aquatic` carries no exemption and governs no rule, so declaring
/// it could never change a verdict — it is the one value whose only possible
/// outcome is another diagnostic.
///
/// CLAUDE.md forbids leaving such a gap to downstream folklore, so the refusal
/// states the gap: routing has one reachability model, standable ground.
#[test]
fn declaring_aquatic_is_dw0455_on_either_body_class() {
    for (label, r) in [
        (
            "npc",
            raw(npcs("0.11.0", Some("aquatic")), quests("0.11.0", None)),
        ),
        (
            "actor",
            raw(npcs("0.11.0", None), quests("0.11.0", Some("aquatic"))),
        ),
    ] {
        let d = check_campaign(&r);
        let hit = d
            .iter()
            .find(|d| d.code == "DW0455")
            .unwrap_or_else(|| panic!("{label}: expected DW0455, got {d:#?}"));
        assert!(
            hit.message.contains("standable ground"),
            "{label}: the refusal must NAME the gap: {}",
            hit.message
        );
        assert!(
            hit.path.ends_with("/traversal/locomotion"),
            "{label}: {hit:?}"
        );
    }
}

/// An unknown locomotion keyword is a schema rejection, not a silently ignored
/// field — `deny_unknown_fields` and a closed enum, like every other DSL
/// vocabulary.
#[test]
fn an_unknown_locomotion_or_field_does_not_parse() {
    for bad in [
        serde_json::json!({ "locomotion": "burrower" }),
        serde_json::json!({ "locomotion": "climber", "opens_gates": true }),
        serde_json::json!({}),
    ] {
        let mut doc: serde_json::Value =
            serde_json::from_str(&common::read_valid("npcs.json")).unwrap();
        doc["dsl_version"] = serde_json::json!("0.11.0");
        doc["content"]["npcs"][0]["traversal"] = bad.clone();
        let r = raw(doc.to_string(), quests("0.11.0", None));
        assert!(
            codes(&r).iter().any(|c| c == "DW0100" || c == "DW0101"),
            "`{bad}` must be rejected by the schema, not accepted: {:#?}",
            check_campaign(&r)
        );
    }
}

/// A campaign that declares nothing is untouched by the whole surface — the
/// additive-superset contract every version ledger entry carries.
#[test]
fn declaring_nothing_at_v11_raises_nothing() {
    let r = raw(npcs("0.11.0", None), quests("0.11.0", None));
    let d = check_campaign(&r);
    assert!(
        !d.iter()
            .any(|d| d.code == "DW0141" || d.code == "DW0454" || d.code == "DW0455"),
        "{d:#?}"
    );
}
