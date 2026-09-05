//! DSL v0.8 (spec-0025): the stage-4 `branch_points` declaration, the per-node
//! `happening`, and the named `campaign-complete` `ending`.
//!
//! This file owns the **structural** half — the version fence (`DW0141`) and the
//! ordinary id/reference rules a branch declaration obeys like every other
//! declaration in the DSL. The proofs *about* branches (`DW0480`–`DW0485`) are
//! compiler-tier and live in `crates/compiler/tests/branch.rs`.

use delvewright_dsl::{Diagnostic, RawCampaign, check_campaign};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

const NPCS: &str = r#"{
  "dsl_version": "0.19.0", "campaign_id": "hello-world", "stage": "npcs",
  "content": { "npcs": [
    { "id": "npc/keeper", "name": "The Keeper", "role": "quest-giver",
      "area": "area/keep", "anchor": "anchor/keeper-stand", "base_entity": "minecraft:villager",
      "persona": { "archetype": "stoic gatekeeper", "speech_style": "Terse.", "motivation": "Guard the gate." } }
  ] }
}"#;

const DIALOGUE: &str = r#"{
  "dsl_version": "0.19.0", "campaign_id": "hello-world", "stage": "dialogue",
  "content": { "dialogues": [
    { "npc": "npc/keeper", "root": "dlg/greeting", "nodes": [
      { "id": "dlg/greeting", "text": "Halt.", "options": [
        { "label": "We hold.",
          "effects": [ { "type": "set-flag", "flag": "flag/wait" },
                       { "type": "complete-objective", "objective": "obj/talk" } ],
          "happening": { "verb": "believes", "text": "The party promises to stand the watch." } },
        { "label": "We run.",
          "effects": [ { "type": "set-flag", "flag": "flag/flee" },
                       { "type": "complete-objective", "objective": "obj/talk" } ],
          "happening": { "verb": "believes", "text": "The party refuses the watch." } } ] } ] }
  ] }
}"#;

/// Stage 5 at `version`, with the fixed two-quest shape.
fn quests(version: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}", "campaign_id": "hello-world", "stage": "quests",
  "content": {{ "quests": [
    {{ "id": "quest/one", "trigger": {{ "type": "campaign-start" }},
       "happening": {{ "verb": "arrives", "text": "The party reaches the door.", "subject": "npc/keeper" }},
       "objectives": [ {{ "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper",
         "happening": {{ "verb": "learns", "text": "The Keeper states the terms." }} }} ],
       "on_complete": [],
       "cast": {{ "npc/keeper": {{ "at": "anchor/keeper-stand", "doing": "barring the door", "dialogue": "dlg/greeting" }} }} }},
    {{ "id": "quest/two", "trigger": {{ "type": "quest-complete", "quest": "quest/one" }},
       "happening": {{ "verb": "departs", "text": "The party leaves." }},
       "objectives": [ {{ "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2,
         "happening": {{ "verb": "departs", "text": "They walk out." }} }} ],
       "on_complete": [ {{ "type": "campaign-complete", "ending": "ending/out",
         "happening": {{ "verb": "gains", "text": "The delve ends on the road." }} }} ],
       "cast": {{ "npc/keeper": {{ "at": "anchor/keeper-stand", "doing": "watching you go", "dialogue": "none" }} }} }}
  ] }}
}}"#
    )
}

/// Stage 4 at `version`, with `branch_points` supplied verbatim.
fn plan(version: &str, branch_points: &str) -> String {
    format!(
        r#"{{
  "dsl_version": "{version}", "campaign_id": "hello-world", "stage": "quest-plan",
  "content": {{ "quests": [
    {{ "id": "quest/one", "goal": "Speak.", "area": "area/keep", "npcs": ["npc/keeper"],
       "depends_on": [], "mandatory": true, "act": 1 }},
    {{ "id": "quest/two", "goal": "Leave.", "area": "area/keep", "npcs": [],
       "depends_on": ["quest/one"], "mandatory": true, "act": 1 }}
  ], "finale": "quest/two", "branch_points": {branch_points} }}
}}"#
    )
}

const POINTS: &str = r#"[
  { "id": "branch-point/gate", "opens_at": "quest/one",
    "forks_on": ["flag/wait", "flag/flee"],
    "branches": [
      { "id": "branch/hold", "flags": ["flag/wait"], "leads_to": "quest/two" },
      { "id": "branch/bolt", "flags": ["flag/flee"], "leads_to": "ending/out" } ] }
]"#;

fn diags(plan_doc: String, quests_doc: String, dialogue_doc: &str) -> Vec<Diagnostic> {
    check_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: NPCS.to_string(),
        classes: hw("classes.json"),
        quest_plan: plan_doc,
        quests: quests_doc,
        dialogue: dialogue_doc.to_string(),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
    })
}

fn green() -> Vec<Diagnostic> {
    diags(plan("0.19.0", POINTS), quests("0.19.0"), DIALOGUE)
}

fn has(d: &[Diagnostic], code: &str) -> bool {
    d.iter().any(|x| x.code == code)
}

/// The reference campaign validates clean — every later test moves one field.
#[test]
fn v08_reference_campaign_is_clean() {
    let d = green();
    assert!(d.is_empty(), "expected a clean campaign, got: {d:#?}");
}

// --- the version fence (DW0141) --------------------------------------------

// --- ordinary declaration rules --------------------------------------------

/// A malformed branch-point / branch id is the ordinary `DW0110`.
#[test]
fn malformed_branch_id_is_dw0110() {
    let bad = POINTS.replace("branch/hold", "hold");
    assert!(has(
        &diags(plan("0.19.0", &bad), quests("0.19.0"), DIALOGUE),
        "DW0110"
    ));
}

/// A `leads_to` that is neither a `quest/…` nor an `ending/…` is `DW0110`: the
/// prefix IS the declaration of which one a branch leads to.
#[test]
fn leads_to_without_a_known_prefix_is_dw0110() {
    let bad = POINTS.replace("\"leads_to\": \"ending/out\"", "\"leads_to\": \"the-road\"");
    let d = diags(plan("0.19.0", &bad), quests("0.19.0"), DIALOGUE);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0110" && x.path.contains("leads_to"))
        .unwrap_or_else(|| panic!("{d:#?}"));
    assert!(hit.message.contains("prefix"), "{}", hit.message);
}

/// Two branches sharing an id collide: branch ids name chronicle files.
#[test]
fn duplicate_branch_id_is_dw0111() {
    let bad = POINTS.replace("branch/bolt", "branch/hold");
    let d = diags(plan("0.19.0", &bad), quests("0.19.0"), DIALOGUE);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0111")
        .unwrap_or_else(|| panic!("{d:#?}"));
    assert!(hit.message.contains("chronicle"), "{}", hit.message);
}

/// `opens_at` / `leads_to` must name real quests, and `ending/…` a real
/// `campaign-complete` — the ordinary dangling-reference rule.
#[test]
fn dangling_branch_reference_is_dw0112() {
    for bad in [
        POINTS.replace(
            "\"opens_at\": \"quest/one\"",
            "\"opens_at\": \"quest/nowhere\"",
        ),
        POINTS.replace(
            "\"leads_to\": \"quest/two\"",
            "\"leads_to\": \"quest/nowhere\"",
        ),
        POINTS.replace(
            "\"leads_to\": \"ending/out\"",
            "\"leads_to\": \"ending/nowhere\"",
        ),
    ] {
        let d = diags(plan("0.19.0", &bad), quests("0.19.0"), DIALOGUE);
        assert!(has(&d, "DW0112"), "{bad}\n{d:#?}");
    }
}

/// A branch may only pin flags its own point forks on.
#[test]
fn branch_flag_outside_forks_on_is_dw0112() {
    let bad = POINTS
        .replace(
            "\"flags\": [\"flag/wait\"]",
            "\"flags\": [\"flag/flee\", \"flag/wait\"]",
        )
        .replace(
            "\"forks_on\": [\"flag/wait\", \"flag/flee\"]",
            "\"forks_on\": [\"flag/wait\"]",
        );
    let d = diags(plan("0.19.0", &bad), quests("0.19.0"), DIALOGUE);
    assert!(has(&d, "DW0112"), "{d:#?}");
}

/// A fork on a flag no `set-flag` produces is the ordinary `DW0172`.
#[test]
fn forking_on_an_unproduced_flag_is_dw0172() {
    let bad = POINTS.replace("flag/flee", "flag/ghost");
    let d = diags(plan("0.19.0", &bad), quests("0.19.0"), DIALOGUE);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0172" && x.path.contains("forks_on"))
        .unwrap_or_else(|| panic!("{d:#?}"));
    assert!(hit.message.contains("not a fork"), "{}", hit.message);
}

/// A `happening.subject` naming nobody cannot be reasoned about, so it is the
/// ordinary dangling-reference error rather than a silently skipped beat.
#[test]
fn dangling_happening_subject_is_dw0112() {
    let bad =
        quests("0.19.0").replace("\"subject\": \"npc/keeper\"", "\"subject\": \"npc/nobody\"");
    let d = diags(plan("0.19.0", POINTS), bad, DIALOGUE);
    let hit = d
        .iter()
        .find(|x| x.code == "DW0112" && x.path.contains("subject"))
        .unwrap_or_else(|| panic!("{d:#?}"));
    assert!(hit.message.contains("npc/nobody"), "{}", hit.message);
}

/// An `item/<kebab>` subject is a free namespace for a story token the campaign
/// tracks by hand, and resolves without a registry.
#[test]
fn item_subject_is_a_free_namespace() {
    let ok = quests("0.19.0").replace(
        "\"subject\": \"npc/keeper\"",
        "\"subject\": \"item/warden-token\"",
    );
    let d = diags(plan("0.19.0", POINTS), ok, DIALOGUE);
    assert!(d.is_empty(), "{d:#?}");
}
