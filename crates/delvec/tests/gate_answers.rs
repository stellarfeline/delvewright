//! **A sealed gate answers for itself** (spec-0117 §3a, §6; criterion 15), on
//! the `souls-shortcut` fixture: a doorway barred from world-load, answered by a
//! `use` trigger with `serves: {gate}` anchored at `anchor/door-leaf`, a cell of
//! the door's own bars.
//!
//! Proven here: the fixture builds with the gate's own answer, and its box rides
//! the bars from the sealed side and goes with `shortcut_open_<id>`; removing the
//! answer reds `DW0429` naming the gate; a knocker beside the door on its far
//! side reds it naming the cell, and a knocker on its sealed side builds, so both
//! forms are reachable; a trigger id in the compiler's `dw-` namespace is
//! `DW0428`; and another box in the door's cell is `DW0422`.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, EnvTrigger, parse_campaign};

fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join("souls-shortcut");
    let loaded = load_campaign_dir(&dir).unwrap();
    parse_campaign(&loaded.raw).expect("souls-shortcut parses")
}

fn try_build(c: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn refusal(c: &Campaign) -> (String, String) {
    match try_build(c) {
        Err(BuildFailure::Diagnostic { code, message }) => (code.to_string(), message),
        Err(other) => panic!("expected a coded refusal, got {other:?}"),
        Ok(_) => panic!("expected a refusal, the campaign built"),
    }
}

fn validation(c: &Campaign) -> Vec<delvewright_dsl::Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    common::validation_diagnostics(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

fn function(out: &BuildOutput, name: &str) -> String {
    let suffix = format!("/function/{name}.mcfunction");
    out.iter()
        .find(|(p, _)| p.starts_with("datapack/") && p.ends_with(&suffix))
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| panic!("no shipped function `{name}`"))
}

fn trigger(json: &str) -> EnvTrigger {
    serde_json::from_str(json).expect("trigger parses")
}

/// The knocker form: the door's own answer replaced by a button beside the
/// door at `at`, and the shortcut's far-side line dropped (no leaf is boxed to
/// say it).
fn with_knocker(at: &str) -> Campaign {
    let mut c = fixture();
    c.quests
        .content
        .triggers
        .retain(|t| t.id.as_str() != "trigger/inner-door-barred");
    c.quests.content.triggers.push(trigger(&format!(
        r#"{{ "id": "trigger/knock", "at": "{at}", "on": {{ "on": "use" }},
              "prop": {{ "block": "minecraft:stone_button[face=floor,facing=north]" }},
              "audience": "presser", "serves": {{ "gate": "anchor/door" }},
              "answers": {{ "done": "The door stands open now." }},
              "effects": [ {{ "type": "narrate", "style": "actionbar", "text": "No one answers the knock." }} ] }}"#
    )));
    if let Some(a) = c.quests.content.shortcuts[0].answers.as_mut() {
        a.not_yet = None;
    }
    c
}

#[test]
fn the_gate_answers_from_its_own_bars_and_goes_when_it_opens() {
    let c = fixture();
    assert!(validation(&c).is_empty(), "{:#?}", validation(&c));
    let out = try_build(&c).expect("the fixture builds with the gate's own answer");
    let arm = function(&out, "seal_arm_door");
    assert!(
        arm.contains("summon minecraft:interaction 4.5 ")
            && arm.contains("\"dw_gate_door\"")
            && arm.contains("dw_trig_inner_door_barred"),
        "the answer box stands over the bars' cell: {arm}"
    );
    let press = function(&out, "press_inner_door_barred");
    assert!(
        press.contains("if entity @s[x="),
        "the gate's own answer speaks to a presser on the sealed side: {press}"
    );
    let open = function(&out, "shortcut_open_inner_door");
    assert!(
        open.contains("kill @e[tag=dw_gate_door]"),
        "the box goes when the bars do: {open}"
    );
}

#[test]
fn a_sealed_gate_with_no_answer_is_dw0429() {
    let mut c = fixture();
    c.quests
        .content
        .triggers
        .retain(|t| t.id.as_str() != "trigger/inner-door-barred");
    if let Some(a) = c.quests.content.shortcuts[0].answers.as_mut() {
        a.not_yet = None;
    }
    let (code, message) = refusal(&c);
    assert_eq!(code, "DW0429", "{message}");
    assert!(message.contains("anchor/door"), "{message}");
}

#[test]
fn a_knocker_on_the_far_side_is_dw0429_and_on_the_sealed_side_builds() {
    let (code, message) = refusal(&with_knocker("anchor/far-knock"));
    assert_eq!(code, "DW0429", "{message}");
    assert!(message.contains("far side"), "{message}");
    try_build(&with_knocker("anchor/door-knock")).expect("a knocker on the sealed side builds");
}

#[test]
fn a_reserved_trigger_id_is_dw0428() {
    let mut c = fixture();
    let mut t = c.quests.content.triggers[0].clone();
    t.id = serde_json::from_str("\"trigger/dw-mine\"").unwrap();
    c.quests.content.triggers.push(t);
    let d = validation(&c);
    assert!(d.iter().any(|d| d.code == "DW0428"), "{d:#?}");
}

#[test]
fn a_second_answer_on_the_doors_cell_is_two_presses_on_one_cell() {
    let mut c = fixture();
    c.quests.content.triggers.push(trigger(
        r#"{ "id": "trigger/second", "at": "anchor/door-leaf", "on": { "on": "use" },
             "audience": "presser", "serves": { "gate": "anchor/door" },
             "effects": [ { "type": "narrate", "style": "actionbar", "text": "Again." } ] }"#,
    ));
    let (code, message) = refusal(&c);
    assert_eq!(code, "DW0878", "{message}");
}

/// `DW0422`: another press's box in the door's answered cell contests the
/// door's own box.
#[test]
fn another_box_in_the_doors_cell_is_dw0422() {
    let mut c = fixture();
    c.quests.content.triggers.push(trigger(
        r#"{ "id": "trigger/rattle", "at": "anchor/door-leaf", "on": { "on": "strike" },
             "prop": { "block": "minecraft:iron_bars" }, "audience": "presser", "once": false,
             "effects": [ { "type": "narrate", "style": "actionbar", "text": "The bars ring." } ] }"#,
    ));
    let (code, message) = refusal(&c);
    assert_eq!(code, "DW0422", "{message}");
    assert!(message.contains("anchor/door"), "{message}");
}

/// Criterion 16: the unlock's highlight is lit only from the side it is
/// pressed from — every box its poll tests lies on the far side of the door
/// (`z > 6`, the side the unlock at `z = 8` stands on), and none on the sealed
/// side; the poll is shut once the shortcut is open.
#[test]
fn the_unlock_is_lit_only_from_its_far_side() {
    let mut c = fixture();
    c.quests.content.shortcuts[0].highlight =
        Some(serde_json::from_str(r#"{ "within": 6 }"#).unwrap());
    let out = try_build(&c).expect("builds with a highlight");
    let poll = function(&out, "hl_poll_p_sc_inner_door");
    let mut boxes = 0;
    for l in poll.lines() {
        let Some(sel) = l.split("@a[").nth(1) else {
            continue;
        };
        assert!(
            l.contains("unless score #sc_inner_door dw.sys matches 1"),
            "the glow is off once the door is open: {l}"
        );
        let field = |k: &str| -> i32 {
            sel.split(',')
                .find_map(|kv| kv.strip_prefix(&format!("{k}=")))
                .and_then(|v| v.trim_end_matches(']').parse().ok())
                .unwrap_or(0)
        };
        let (z, dz) = (field("z"), field("dz"));
        assert!(z >= 7 && z + dz >= 7, "a lit box on the sealed side: {l}");
        boxes += 1;
    }
    assert!(boxes > 0, "the lit set is not empty: {poll}");
}
