//! spec-0140, document tier: a mode's shape (`DW1122`), every reference to a
//! mode and a kept quest resolving (`DW0112`), and no kept objective waiting
//! on a flag only held quests set (`DW1123`).

use delvewright_dsl::{DSL_VERSION, RawCampaign, parse_campaign, validate_campaign};
use serde_json::{Value, json};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

/// hello-world with `edit` applied to its stage-5 `content`.
fn with(edit: impl FnOnce(&mut Value)) -> delvewright_dsl::Campaign {
    let mut q: Value = serde_json::from_str(&hw("quests.json")).unwrap();
    q["dsl_version"] = json!(DSL_VERSION);
    edit(&mut q["content"]);
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: q.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("parses")
}

fn codes_at(c: &delvewright_dsl::Campaign, code: &str) -> Vec<String> {
    validate_campaign(c)
        .into_iter()
        .filter(|d| d.code == code)
        .map(|d| d.path)
        .collect()
}

const MAIN: &str = "quest/open-the-door";

fn mode(keeps: &[&str]) -> Value {
    json!({"id": "mode/aside", "keeps": keeps})
}

/// A second quest, `quest/side`, whose completion sets `flag/side-done`.
fn side() -> Value {
    json!({
        "id": "quest/side",
        "trigger": {"type": "campaign-start"},
        "objectives": [
            {"type": "reach-anchor", "id": "obj/side", "anchor": "anchor/exit", "radius": 2}
        ],
        "on_complete": [{"type": "set-flag", "flag": "flag/side-done"}]
    })
}

fn on_talk(content: &mut Value, effects: Value) {
    content["quests"][0]["on_objective_complete"]["obj/talk"] = effects;
}

fn enter() -> Value {
    json!({"type": "enter-mode", "mode": "mode/aside"})
}

#[test]
fn a_mode_as_written_is_accepted_and_numbered_by_its_place() {
    let c = with(|q| {
        q["quests"].as_array_mut().unwrap().push(side());
        q["modes"] = json!([mode(&[MAIN]), {"id": "mode/other", "keeps": ["quest/side"]}]);
        on_talk(
            q,
            json!([enter(), {"type": "leave-mode", "mode": "mode/aside"}]),
        );
    });
    for code in ["DW1122", "DW1123", "DW0112", "DW0110", "DW0111"] {
        assert!(codes_at(&c, code).is_empty(), "{code}");
    }
    let q = &c.quests.content;
    assert_eq!(q.mode_number("mode/aside"), Some(1));
    assert_eq!(q.mode_number("mode/other"), Some(2));
    assert_eq!(q.mode_number("mode/none"), None);
    // The main quest is held by the second mode alone, the side quest by the
    // first.
    assert_eq!(q.modes_holding(MAIN), [2]);
    assert_eq!(q.modes_holding("quest/side"), [1]);
    // A campaign with no mode writes no `modes`.
    let none = with(|_| {});
    let wire = serde_json::to_value(&none.quests.content).unwrap();
    assert!(wire.get("modes").is_none(), "{wire}");
}

#[test]
fn a_mode_that_cannot_be_run_is_dw1122() {
    let empty = with(|q| q["modes"] = json!([mode(&[])]));
    assert_eq!(codes_at(&empty, "DW1122"), ["/content/modes/0/keeps"]);
    let twice = with(|q| q["modes"] = json!([mode(&[MAIN, MAIN])]));
    assert_eq!(codes_at(&twice, "DW1122"), ["/content/modes/0/keeps/1"]);
    let once = with(|q| q["modes"] = json!([mode(&[MAIN])]));
    assert!(codes_at(&once, "DW1122").is_empty());
}

#[test]
fn every_reference_to_a_mode_resolves() {
    let c = with(|q| {
        q["modes"] = json!([mode(&["quest/nowhere"])]);
        on_talk(
            q,
            json!([{"type": "enter-mode", "mode": "mode/nowhere"},
                   {"type": "leave-mode", "mode": "mode/nowhere"}]),
        );
    });
    let mut at = codes_at(&c, "DW0112");
    at.sort();
    assert_eq!(
        at,
        [
            "/content/modes/0/keeps/0",
            "/content/quests/0/on_objective_complete/obj/talk/0",
            "/content/quests/0/on_objective_complete/obj/talk/1",
        ]
    );
    let dup = with(|q| q["modes"] = json!([mode(&[MAIN]), mode(&[MAIN])]));
    assert!(!codes_at(&dup, "DW0111").is_empty());
    let bad = with(|q| q["modes"] = json!([{"id": "mode/Not_Kebab", "keeps": [MAIN]}]));
    assert!(!codes_at(&bad, "DW0110").is_empty());
}

/// The kept quest's `obj/exit` waits on `flag/side-done`, which only the held
/// quest's completion sets.
fn waits_on_the_side(q: &mut Value) {
    q["quests"].as_array_mut().unwrap().push(side());
    q["quests"][0]["objectives"][1]["requires_flags"] = json!(["flag/side-done"]);
    q["modes"] = json!([mode(&[MAIN])]);
}

#[test]
fn a_kept_objective_waiting_on_a_flag_only_held_quests_set_is_dw1123() {
    let at = "/content/quests/0/objectives/1/requires_flags/0";
    let held = with(|q| {
        waits_on_the_side(q);
        on_talk(q, json!([enter()]));
    });
    assert_eq!(codes_at(&held, "DW1123"), [at]);
    // Keeping the quest that sets it.
    let kept = with(|q| {
        waits_on_the_side(q);
        q["modes"][0]["keeps"] = json!([MAIN, "quest/side"]);
        on_talk(q, json!([enter()]));
    });
    assert!(codes_at(&kept, "DW1123").is_empty());
    // A trigger sets it too: no mode holds a trigger.
    let free = with(|q| {
        waits_on_the_side(q);
        on_talk(q, json!([enter()]));
        q["triggers"] = json!([{
            "id": "trigger/side-door", "at": "anchor/exit", "on": {"on": "strike"},
            "effects": [{"type": "set-flag", "flag": "flag/side-done"}]
        }]);
    });
    assert!(codes_at(&free, "DW1123").is_empty());
    // A mode nothing enters holds nothing.
    let unentered = with(waits_on_the_side);
    assert!(codes_at(&unentered, "DW1123").is_empty());
    // The held quest that sets it is complete before the kept one starts.
    let before = with(|q| {
        waits_on_the_side(q);
        on_talk(q, json!([enter()]));
        q["quests"][0]["trigger"] = json!({"type": "quest-complete", "quest": "quest/side"});
    });
    assert!(codes_at(&before, "DW1123").is_empty());
}

#[test]
fn an_objective_settled_before_every_entry_is_not_open_under_the_mode() {
    // The gated objective is the one whose bundle enters the mode: it is done
    // before the mode is on.
    let behind = with(|q| {
        q["quests"].as_array_mut().unwrap().push(side());
        q["quests"][0]["objectives"][0]["requires_flags"] = json!(["flag/side-done"]);
        q["modes"] = json!([mode(&[MAIN])]);
        on_talk(q, json!([enter()]));
    });
    assert!(codes_at(&behind, "DW1123").is_empty());
    // A second entry, from the held quest's bundle, settles nothing first.
    let elsewhere = with(|q| {
        q["quests"].as_array_mut().unwrap().push(side());
        q["quests"][0]["objectives"][0]["requires_flags"] = json!(["flag/side-done"]);
        q["modes"] = json!([mode(&[MAIN])]);
        on_talk(q, json!([enter()]));
        q["quests"][1]["on_objective_complete"] = json!({"obj/side": [enter()]});
    });
    assert_eq!(
        codes_at(&elsewhere, "DW1123"),
        ["/content/quests/0/objectives/0/requires_flags/0"]
    );
}
