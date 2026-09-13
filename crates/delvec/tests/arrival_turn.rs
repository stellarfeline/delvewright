//! A walked body takes its arrival turn — **whichever verb walked it**.
//!
//! The rule (`nav::apply_arrival_yaw`): the last waypoint of a walk does not
//! carry the bearing of the final step, because a body that walked away from the
//! party would arrive with its back to them. It carries the destination anchor's
//! declared `facing` when the piece declares one, and otherwise the reverse of
//! the last leg — the way back down the path it just walked, where whoever
//! followed it is standing.
//!
//! `move-npc` and `move-actor` are one rule over one object class, so both are
//! pinned here, in one file, over one fixture, in both branches:
//!
//! | verb | destination | branch |
//! |---|---|---|
//! | `move-npc` | `anchor/exit` (hello-room declares no `facing`) | reversed |
//! | `move-actor` | `anchor/exit` | reversed |
//! | `move-actor` | `anchor/keeper-stand` (`facing: north`) | declared |
//!
//! The reversal rows are what prove the rule fires at all — on this fixture the
//! declared row's answer happens to coincide with the path tangent, so it
//! separates the two branches from each other and nothing more. Each test says
//! which question it answers.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::json;

const FN_DIR: &str = "datapack/data/hello-world/function";

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world with a walker the story moves twice and an NPC it moves once.
///
/// The keeper walks to `anchor/exit`; the walker puppet is spawned, walks to
/// `anchor/exit`, then walks back to `anchor/keeper-stand`. Two destinations,
/// one of which the prefab gives a `facing` and one of which it does not — so
/// both halves of the rule are exercised without a second fixture.
fn campaign() -> Campaign {
    let quests = common::patch_doc(&read_hw("quests.json"), |doc| {
        doc["content"]["actors"] = json!([{
            "id": "actor/walker",
            "entity": "minecraft:villager",
            "anchor": "spawn"
        }]);
        let effects = common::objective_effects(doc, 0, "obj/talk");
        effects.push(json!({
            "type": "spawn-actor",
            "actor": "actor/walker",
            "happening": { "verb": "arrives", "text": "the walker steps out" }
        }));
        effects.push(json!({
            "type": "move-actor",
            "actor": "actor/walker",
            "to_anchor": "anchor/exit",
            "happening": { "verb": "arrives", "text": "the walker crosses to anchor/exit" }
        }));
        effects.push(json!({
            "type": "move-actor",
            "actor": "actor/walker",
            "to_anchor": "anchor/keeper-stand",
            "happening": { "verb": "arrives", "text": "the walker returns to anchor/keeper-stand" }
        }));
        effects.push(json!({
            "type": "move-npc",
            "npc": "npc/keeper",
            "to_anchor": "anchor/exit",
            "happening": { "verb": "arrives", "text": "the keeper crosses to anchor/exit" }
        }));
    });
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests,
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

fn build(campaign: &Campaign, prefabs: &PrefabRegistry) -> BuildOutput {
    let plan = Plan::build(campaign, prefabs).expect("plan builds");
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
        prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("every emitted command validates")
}

/// Every `… run tp <target> <x> <y> <z> <yaw> <pitch>` yaw in a walk driver, in
/// tick order — the body's facing, one entry per tick.
fn walk_yaws(out: &BuildOutput, driver: &str) -> Vec<i32> {
    let path = format!("{FN_DIR}/{driver}.mcfunction");
    let text = out
        .iter()
        .find(|(p, _)| p.as_str() == path)
        .map(|(_, b)| String::from_utf8(b.clone()).unwrap())
        .unwrap_or_else(|| {
            let drivers: Vec<&str> = out
                .keys()
                .map(|p| p.as_str())
                .filter(|p| p.contains("/ma_tick_") || p.contains("/mv_tick_"))
                .collect();
            panic!("expected walk driver `{path}`; this build emitted {drivers:?}")
        });
    let yaws: Vec<i32> = text
        .lines()
        .filter(|l| l.contains(" run tp "))
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            f[f.len() - 2]
                .parse()
                .unwrap_or_else(|_| panic!("a walked tp carries a yaw: {l}"))
        })
        .collect();
    assert!(
        yaws.len() > 2,
        "`{driver}` is not a multi-tick walk ({} tp line(s)), so it cannot show an \
         arrival turn at all",
        yaws.len()
    );
    yaws
}

/// A walk whose destination anchor declares no `facing` ends with the body turned
/// through 180 degrees, facing back down the path it walked. Both verbs.
#[test]
fn an_undeclared_destination_turns_the_body_back_the_way_it_came() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&campaign(), &prefabs);

    // `anchor/exit` in `hello-room` declares no `facing`, so the reversal branch
    // is what answers for BOTH movers walking to it.
    for driver in ["mv_tick_keeper_exit", "ma_tick_walker_exit"] {
        let yaws = walk_yaws(&out, driver);
        let arrival = *yaws.last().unwrap();
        let approach = yaws[yaws.len() - 2];
        assert_eq!(
            arrival,
            (approach + 180).rem_euclid(360),
            "`{driver}` leaves the body on its path tangent {approach} instead of turning \
             it back to {}: a walked body that stops with its back to the party is the \
             defect this rule exists for. Full driver yaws: {yaws:?}",
            (approach + 180).rem_euclid(360)
        );
    }
}

/// A walk whose destination anchor declares a `facing` ends with the body in that
/// facing — the anchor is where the piece says a body at that spot looks, and it
/// outranks the reversal default.
///
/// **What this separates.** The walker's return leg approaches
/// `anchor/keeper-stand` heading north, and that anchor declares `facing: north`,
/// so the declared answer coincides with the path tangent and this test alone
/// cannot tell the rule from its absence — that is
/// [`an_undeclared_destination_turns_the_body_back_the_way_it_came`]'s job. What
/// it does separate is the branch: an actor destination whose declared facing the
/// planner never reads falls through to the reversal (yaw 0 here), which is what
/// the second assertion refuses.
#[test]
fn a_declared_destination_facing_wins_over_the_reversal_default() {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let out = build(&campaign(), &prefabs);

    // `anchor/keeper-stand` in `hello-room` declares `facing: north` (MC yaw 180).
    let yaws = walk_yaws(&out, "ma_tick_walker_keeper_stand");
    let arrival = *yaws.last().unwrap();
    let approach = yaws[yaws.len() - 2];
    let reversed = (approach + 180).rem_euclid(360);
    assert_eq!(
        arrival, 180,
        "the walker arrives on `anchor/keeper-stand`, which declares `facing: north`, \
         but the driver leaves it at yaw {arrival}. Full driver yaws: {yaws:?}"
    );
    assert_ne!(
        arrival, reversed,
        "the walker's arrival yaw is the reversal default, so the destination \
         anchor's declared `facing` reached the actor planner as `None` — \
         `nav::actor_anchor_facing_yaw` resolved nothing. Full driver yaws: {yaws:?}"
    );
}
