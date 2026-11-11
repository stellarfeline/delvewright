//! spec-0134 §8 — a trigger on an actor's body. `strike-actor` and
//! `use-actor` are the actor's twins of `strike-npc`: they ride the actor's
//! answer box, which the actor gains when a trigger names it. Proven over
//! hello-world with a fox set out at the keeper's talk.
//!
//! The taking (`give-item` gaining `from: {actor, slot}`) is not in this
//! round: a recorded debt.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

const FN_DIR: &str = "datapack/data/hello-world/function";
const ADV_DIR: &str = "datapack/data/hello-world/advancement";

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn fox(vulnerable: bool) -> Value {
    json!({
        "id": "actor/fox", "entity": "minecraft:fox", "anchor": "spawn",
        "offset": [2, 0, 0], "facing": "south", "vulnerable": vulnerable,
        "equipment": { "main_hand": "minecraft:tripwire_hook" }
    })
}

fn raw(vulnerable: bool, triggers: Vec<Value>) -> RawCampaign {
    let quests = common::patch_doc(&read_hw("quests.json"), |doc| {
        doc["content"]["actors"] = json!([fox(vulnerable)]);
        common::objective_effects(doc, 0, "obj/talk").push(json!({
            "type": "spawn-actor", "actor": "actor/fox",
            "happening": { "verb": "arrives", "text": "the fox is set out" }
        }));
        doc["content"]["triggers"] = Value::Array(triggers);
    });
    RawCampaign {
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
    }
}

fn on_fox(id: &str, on: &str, actor: &str) -> Value {
    json!({
        "id": id, "on": {"on": on, "actor": actor}, "audience": "presser", "once": false,
        "effects": [{"type": "narrate", "text": "The fox flinches.", "style": "actionbar"}]
    })
}

fn campaign(r: &RawCampaign) -> Campaign {
    parse_campaign(r).unwrap_or_else(|d| panic!("campaign parses: {d:#?}"))
}

fn errors(c: &Campaign) -> Vec<(String, String)> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    common::validation_diagnostics(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .iter()
    .filter(|d| d.severity == delvewright_dsl::Severity::Error)
    .map(|d| (d.code.to_string(), d.path.clone()))
    .collect()
}

fn build(c: &Campaign) -> BuildOutput {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut s = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                s.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &BTreeMap::new(),
        &s,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| panic!("build fails: {e:?}"))
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(out.get(path).unwrap_or_else(|| panic!("no {path}")).clone()).unwrap()
}

#[test]
fn strike_actor_and_use_actor_ride_the_actors_answer_box() {
    let c = campaign(&raw(
        false,
        vec![
            on_fox("trigger/cuff-the-fox", "strike-actor", "actor/fox"),
            on_fox("trigger/pet-the-fox", "use-actor", "actor/fox"),
        ],
    ));
    assert!(errors(&c).is_empty(), "{:#?}", errors(&c));
    let out = build(&c);
    let spawn = text(&out, &format!("{FN_DIR}/spawn_actor_fox.mcfunction"));
    let summon = spawn
        .lines()
        .find(|l| l.contains("summon minecraft:interaction"))
        .unwrap_or_else(|| panic!("the fox gains an answer box:\n{spawn}"));
    assert!(summon.contains("\"dw_abody_fox\""), "{summon}");
    assert!(summon.contains("\"dw_trig_cuff_the_fox\""), "{summon}");
    assert!(summon.contains("\"dw_trig_pet_the_fox\""), "{summon}");
    let blow = text(&out, &format!("{ADV_DIR}/press_cuff_the_fox.json"));
    assert!(blow.contains("minecraft:player_hurt_entity"), "{blow}");
    assert!(blow.contains("dw_trig_cuff_the_fox"), "{blow}");
    let touch = text(&out, &format!("{ADV_DIR}/press_pet_the_fox.json"));
    assert!(
        touch.contains("minecraft:player_interacted_with_entity"),
        "{touch}"
    );
}

#[test]
fn an_actor_no_trigger_names_and_no_answer_declares_wears_no_box() {
    let c = campaign(&raw(false, Vec::new()));
    let out = build(&c);
    let spawn = text(&out, &format!("{FN_DIR}/spawn_actor_fox.mcfunction"));
    assert!(!spawn.contains("minecraft:interaction"), "{spawn}");
}

#[test]
fn a_trigger_on_an_undeclared_actor_is_dw0112() {
    let c = campaign(&raw(
        false,
        vec![on_fox("trigger/cuff", "strike-actor", "actor/badger")],
    ));
    assert!(
        errors(&c).contains(&(
            "DW0112".to_string(),
            "/content/triggers/0/on/actor".to_string()
        )),
        "{:#?}",
        errors(&c)
    );
}

#[test]
fn a_trigger_on_a_vulnerable_actor_is_dw1056() {
    let c = campaign(&raw(
        true,
        vec![on_fox("trigger/pet", "use-actor", "actor/fox")],
    ));
    assert!(
        errors(&c).contains(&(
            "DW1056".to_string(),
            "/content/triggers/0/on/actor".to_string()
        )),
        "{:#?}",
        errors(&c)
    );
}

#[test]
fn a_trigger_on_an_actor_with_an_anchor_is_dw0194() {
    let mut t = on_fox("trigger/pet", "use-actor", "actor/fox");
    t["at"] = json!("spawn");
    let c = campaign(&raw(false, vec![t]));
    assert!(
        errors(&c).contains(&("DW0194".to_string(), "/content/triggers/0/at".to_string())),
        "{:#?}",
        errors(&c)
    );
}
