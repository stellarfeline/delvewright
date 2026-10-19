//! `DW1050`: a cast scene that outlives its beat is refused.
//!
//! Fixture `talk-after-end`: the keeper's farewell is the last beat, and its
//! scene opens a two-node tree (`dlg/farewell` → `dlg/last-word`) whose second
//! node completes `obj/farewell`, which completes the campaign. Nothing replaces
//! that scene afterwards, so right-click re-opens the finished beat. The opening
//! beat (`dlg/greeting`, completing `obj/ask`) is replaced by the farewell
//! quest's own cast, which is the successor case.

mod common;

use delvec::compiler::cast::{self, OutlivedBinding};
use delvewright_dsl::{Campaign, parse_campaign};
use serde_json::{Value, json};

fn load() -> (Value, Value) {
    let dir = common::compiler_fixtures_dir().join("talk-after-end");
    let read = |f: &str| -> Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(f)).unwrap()).unwrap()
    };
    (read("quests.json"), read("dialogue.json"))
}

fn campaign(quests: &Value, dialogue: &Value) -> Campaign {
    let dir = common::compiler_fixtures_dir().join("talk-after-end");
    let mut loaded = delvec::compiler::load::load_campaign_dir(&dir).unwrap();
    loaded.raw.quests = serde_json::to_string(quests).unwrap();
    loaded.raw.dialogue = serde_json::to_string(dialogue).unwrap();
    parse_campaign(&loaded.raw).expect("fixture parses")
}

fn run(quests: &Value, dialogue: &Value) -> (Vec<delvewright_dsl::Diagnostic>, OutlivedBinding) {
    cast::check_outlived_scenes(&campaign(quests, dialogue))
}

/// The option that completes `obj/farewell` also sets `flag/farewell-said`.
fn completer_sets_flag(dialogue: &mut Value) {
    let nodes = dialogue["content"]["dialogues"][0]["nodes"]
        .as_array_mut()
        .unwrap();
    let last = nodes
        .iter_mut()
        .find(|n| n["id"] == "dlg/last-word")
        .unwrap();
    last["options"][0]["effects"]
        .as_array_mut()
        .unwrap()
        .insert(
            0,
            json!({ "type": "set-flag", "flag": "flag/farewell-said" }),
        );
}

/// `quest/farewell`'s keeper entry becomes `[the beat, unless said; after, once said]`.
fn after_placement(quests: &mut Value, after: Value) {
    let q = &mut quests["content"]["quests"][1];
    assert_eq!(q["id"], "quest/farewell");
    let mut beat = q["cast"]["npc/keeper"].clone();
    beat["forbids_flags"] = json!(["flag/farewell-said"]);
    let mut later = json!({
        "at": beat["at"].clone(),
        "doing": "watching the road close behind the party",
        "requires_flags": ["flag/farewell-said"]
    });
    later["dialogue"] = after;
    q["cast"]["npc/keeper"] = json!([beat, later]);
}

/// Red before the fix: the last beat's scene, which nothing replaces, is refused,
/// naming the NPC, the clause and the objective. The opening beat is not.
#[test]
fn dw1050_the_last_beat_that_nothing_replaces_is_refused() {
    let (q, d) = load();
    let (diags, bind) = run(&q, &d);
    let hits: Vec<_> = diags.iter().filter(|d| d.code == "DW1050").collect();
    assert_eq!(hits.len(), 1, "{diags:#?}");
    let m = &hits[0].message;
    for s in [
        "npc/keeper",
        "quest/farewell",
        "dlg/farewell",
        "obj/farewell",
    ] {
        assert!(m.contains(s), "message must name `{s}`: {m}");
    }
    assert!(
        !m.contains("obj/ask"),
        "the replaced opening beat is not refused: {m}"
    );
    assert_eq!(hits[0].path, "/content/quests/1/cast/npc/keeper");
    assert_eq!(
        bind,
        OutlivedBinding {
            judged: 2,
            with_completer: 2,
            refused: 1
        }
    );
    assert!(
        bind.line().contains("2 cast clause(s) judged"),
        "{}",
        bind.line()
    );
}

/// The remedy: a placement after the beat, gated on a flag the completing option
/// sets. A bark pool passes.
#[test]
fn dw1050_a_post_completion_placement_passes() {
    let (mut q, mut d) = load();
    completer_sets_flag(&mut d);
    after_placement(&mut q, json!({ "barks": ["The gate is down. Go."] }));
    let (diags, bind) = run(&q, &d);
    assert!(!diags.iter().any(|d| d.code == "DW1050"), "{diags:#?}");
    assert_eq!(bind.refused, 0);
    assert_eq!(bind.judged, 3);
}

/// Repeating the beat is a stated choice: the same root under the gate passes.
#[test]
fn dw1050_repeating_the_beat_under_the_gate_is_a_declaration() {
    let (mut q, mut d) = load();
    completer_sets_flag(&mut d);
    after_placement(&mut q, json!("dlg/farewell"));
    let (diags, _) = run(&q, &d);
    assert!(!diags.iter().any(|d| d.code == "DW1050"), "{diags:#?}");
}

/// A flag that nothing gates on is not a declaration: setting it alone does not pass.
#[test]
fn dw1050_a_flag_no_placement_reads_does_not_pass() {
    let (q, mut d) = load();
    completer_sets_flag(&mut d);
    let (diags, _) = run(&q, &d);
    assert!(diags.iter().any(|d| d.code == "DW1050"), "{diags:#?}");
}

/// The successor case: the opening beat passes because `quest/farewell`, started
/// by `quest/ask` completing, casts the keeper. Take that cast row away and the
/// opening beat is what governs after `obj/ask`, and it is refused.
#[test]
fn dw1050_a_successor_quest_that_casts_the_npc_passes() {
    let (mut q, d) = load();
    let (diags, _) = run(&q, &d);
    assert!(
        !diags
            .iter()
            .any(|d| d.code == "DW1050" && d.message.contains("obj/ask")),
        "{diags:#?}"
    );
    q["content"]["quests"][1]["cast"]
        .as_object_mut()
        .unwrap()
        .remove("npc/keeper");
    let (diags, _) = run(&q, &d);
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW1050" && d.message.contains("obj/ask")),
        "{diags:#?}"
    );
}

/// Retiring the beat is not saying what comes after: a `forbids` term on the
/// refused placement itself only hands right-click back to an earlier clause
/// (here the opening beat), so it does not pass.
#[test]
fn dw1050_forbidding_the_flag_on_the_beat_alone_does_not_pass() {
    let (mut q, mut d) = load();
    completer_sets_flag(&mut d);
    q["content"]["quests"][1]["cast"]["npc/keeper"]["forbids_flags"] =
        json!(["flag/farewell-said"]);
    let (diags, _) = run(&q, &d);
    assert!(diags.iter().any(|d| d.code == "DW1050"), "{diags:#?}");
}
