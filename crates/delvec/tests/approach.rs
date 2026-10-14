//! **A body the party talks to faces the party as they walk up** (`DW1025`,
//! `compiler::approach`).
//!
//! hello-world's keeper is talked to at `anchor/keeper-stand`, and the route
//! proof delivers the party into that cell from the west (`[4, 65, 4]` into
//! `[5, 65, 4]`). The pinned prefab declares the stand facing `north`, which is
//! side-on to that approach — exactly on the front half-plane's edge, which is
//! closed. Each case varies the one thing the rule reads, the anchor's declared
//! facing, in a scratch copy of the prefab's metadata: `east` turns the keeper's
//! back to the party, `west` faces them, and a `watch` drawn within reach of the
//! approach answers a back-facing stand.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// A prefab directory holding hello-room with `anchor/keeper-stand` facing `facing`.
fn prefabs(facing: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("approach-prefabs-{facing}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = common::prefabs_dir();
    std::fs::copy(src.join("hello-room.nbt"), dir.join("hello-room.nbt")).unwrap();
    let mut meta: Value =
        serde_json::from_str(&std::fs::read_to_string(src.join("hello-room.json")).unwrap())
            .unwrap();
    assert_eq!(
        meta["anchors"]["anchor/keeper-stand"]["facing"], "north",
        "the pinned stand's facing is the baseline every case here varies"
    );
    meta["anchors"]["anchor/keeper-stand"]["facing"] = json!(facing);
    std::fs::write(
        dir.join("hello-room.json"),
        serde_json::to_string_pretty(&meta).unwrap(),
    )
    .unwrap();
    dir
}

fn campaign(npcs: impl FnOnce(&mut Value)) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: common::patch_doc(&read_hw("npcs.json"), npcs),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: read_hw("quests.json"),
        dialogue: read_hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

/// The build's verdict: `Ok` or the refusing code and message.
fn build(c: &Campaign, facing: &str) -> Result<(), (String, String)> {
    let dir = prefabs(facing);
    let reg = PrefabRegistry::load_dir(&dir).expect("prefabs load");
    let plan = Plan::build(c, &reg).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                structures.insert(
                    t.structure_file.clone(),
                    std::fs::read(dir.join(&t.structure_file)).unwrap(),
                );
            }
        }
    }
    match emit::build(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &reg,
        None,
        &BTreeMap::new(),
    ) {
        Ok(_) => Ok(()),
        Err(BuildFailure::Diagnostic { code, message }) => Err((code.to_string(), message)),
        Err(other) => panic!("the build failed outside a diagnostic: {other:?}"),
    }
}

fn watch(who: Value, within: u32) -> impl FnOnce(&mut Value) {
    move |n: &mut Value| {
        n["content"]["npcs"][0]["watch"] = json!({ "who": who, "within": within });
    }
}

#[test]
fn a_side_on_keeper_faces_the_party_and_builds() {
    assert_eq!(build(&campaign(|_| {}), "north"), Ok(()));
}

#[test]
fn a_keeper_facing_the_approach_builds() {
    assert_eq!(build(&campaign(|_| {}), "west"), Ok(()));
}

#[test]
fn a_keeper_with_its_back_to_the_approach_is_refused() {
    let (code, message) = build(&campaign(|_| {}), "east").expect_err("refused");
    assert_eq!(code, "DW1025", "{message}");
    for needle in [
        "`npc/keeper`",
        "`obj/talk`",
        "the critical path",
        "[4, 65, 4]",
        "180 degrees",
        "\"within\": 3",
    ] {
        assert!(message.contains(needle), "missing {needle}: {message}");
    }
}

#[test]
fn a_watch_that_reaches_the_approach_answers_a_back_turned_stand() {
    assert_eq!(build(&campaign(watch(json!("nearest"), 3)), "east"), Ok(()));
}

#[test]
fn a_class_watch_does_not_answer_it() {
    // The party member who talks may be of another class, so the stand is
    // judged by its own yaw.
    let c = campaign(watch(json!({ "class": "class/wanderer" }), 3));
    let (code, _) = build(&c, "east").expect_err("refused");
    assert_eq!(code, "DW1025");
}
