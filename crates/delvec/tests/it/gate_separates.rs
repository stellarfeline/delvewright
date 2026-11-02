//! `DW1011` on a whole build: a shut gate the story operates separates its two
//! sides, a gate the story declares a way round is not judged, and a gate no
//! beat operates keeps nothing behind it.
//!
//! The subject is the `souls-shortcut` fixture: a keep whose inner wall holds
//! `anchor/door`, with a way round the wall carved by its own world edits — the
//! long way the shortcut shortens. The three tests differ in one declaration
//! each, and the ledger says which population each one fell into.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, QuestEffect, parse_campaign};

fn fixture() -> Campaign {
    let dir = common::compiler_fixtures_dir().join("souls-shortcut");
    let loaded = load_campaign_dir(&dir).unwrap();
    parse_campaign(&loaded.raw).expect("souls-shortcut parses")
}

fn try_build(campaign: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
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

fn ledger(out: &BuildOutput) -> serde_json::Value {
    serde_json::from_slice(
        out.get("validation/separation-proof.json")
            .expect("the separation ledger is emitted on a build that walks"),
    )
    .unwrap()
}

/// As shipped: the door is the shortcut's, and the carved way round is the long
/// way it shortens, by declaration.
#[test]
fn a_shortcut_door_and_its_long_way_round_is_not_judged() {
    let out = try_build(&fixture()).expect("the fixture builds");
    let l = ledger(&out);
    assert_eq!(l["code"], "DW1011", "{l}");
    assert_eq!(l["gates"], 1, "{l}");
    assert_eq!(l["shortcuts"], 1, "{l}");
    assert_eq!(l["bypassed"], 0, "{l}");
}

/// The shortcut taken away and nothing else: the door is shut from world load
/// and no beat opens or closes it, so no beat is kept behind it and the way
/// round is simply the way.
#[test]
fn a_door_no_beat_operates_keeps_nothing_behind_it() {
    let mut c = fixture();
    c.quests.content.shortcuts.clear();
    // The door's two answers belong to the shortcut's hitboxes; without it
    // they would stand coincident on one cell (`DW0878`).
    c.quests.content.triggers.clear();
    let out = try_build(&c).expect("a door nothing opens is a wall with bars");
    let l = ledger(&out);
    assert_eq!(l["unoperated"], 1, "{l}");
    assert_eq!(
        (l["shut"].as_u64(), l["bypassed"].as_u64()),
        (Some(0), Some(0)),
        "{l}"
    );
}

/// The same door made a story gate — the keeper's talk opens it — beside the
/// carved way round: a body walks round it before the talk, and the build
/// refuses it naming the gate and the walk.
#[test]
fn a_door_the_story_opens_beside_a_way_round_is_refused_dw1011() {
    let mut c = fixture();
    c.quests.content.shortcuts.clear();
    // The door's two answers belong to the shortcut's hitboxes; without it
    // they would stand coincident on one cell (`DW0878`).
    c.quests.content.triggers.clear();
    let open: QuestEffect = serde_json::from_str(
        r#"{ "type": "open-gate", "anchor": "anchor/door",
             "happening": { "text": "the gate at anchor/door opens", "verb": "opens" } }"#,
    )
    .expect("open-gate parses");
    c.quests.content.quests[0]
        .on_objective_complete
        .get_mut(&delvewright_dsl::ObjectiveId("obj/talk".to_string()))
        .expect("obj/talk carries effects")
        .push(open);
    let Err(BuildFailure::Diagnostic { code, message }) = try_build(&c) else {
        panic!("a story gate with a way round must be refused");
    };
    assert_eq!(code.id(), "DW1011", "{message}");
    assert!(message.contains("gate `anchor/door`"), "{message}");
    assert!(message.contains("walking x"), "{message}");
    assert!(message.contains("near face"), "{message}");
}
