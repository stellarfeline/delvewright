//! A box steps aside for the body's own use: an actor's answer box encloses
//! its body and takes every right-click meant for it, so while a gathered
//! `collect` whose item a use on a body of that kind yields is open, the box is
//! not standing (`bodies::owed_uses`). Proven over hello-world with an axolotl
//! set out at the keeper's talk and a collect of its bucket.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

const FN_DIR: &str = "datapack/data/hello-world/function";

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// Hello-world with `actor/pip`, a boxed body of `kind`, spawned at the talk,
/// and `obj/scoop`, a gathered collect of `item` after the talk.
fn raw(kind: &str, item: &str) -> RawCampaign {
    let quests = common::patch_doc(&read_hw("quests.json"), |doc| {
        doc["content"]["actors"] = json!([{
            "id": "actor/pip", "entity": kind, "anchor": "spawn",
            "offset": [2, 0, 0], "facing": "south",
            "answers": { "touched": "Pip peers at you.", "struck": "Pip blows a bubble." }
        }]);
        common::objective_effects(doc, 0, "obj/talk").push(json!({
            "type": "spawn-actor", "actor": "actor/pip",
            "happening": { "verb": "arrives", "text": "Pip is set out" }
        }));
        let objectives = doc["content"]["quests"][0]["objectives"]
            .as_array_mut()
            .unwrap();
        objectives.insert(
            1,
            json!({
                "type": "collect", "id": "obj/scoop", "after": ["obj/talk"],
                "item": item, "count": 1, "anchor": "spawn", "gathered": true,
                "happening": { "verb": "gains", "text": "the party scoop Pip" }
            }),
        );
        objectives[2]["after"] = json!(["obj/scoop"]);
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

fn function(out: &BuildOutput, name: &str) -> Option<String> {
    out.get(&format!("{FN_DIR}/{name}.mcfunction"))
        .map(|b| String::from_utf8(b.clone()).unwrap())
}

fn summons_box(line: &str) -> bool {
    line.contains("summon minecraft:interaction") && line.contains("\"dw_abody_pip\"")
}

#[test]
fn a_box_over_a_body_a_gathered_use_is_owed_on_steps_aside_while_the_collect_is_open() {
    let c = campaign(&raw("minecraft:axolotl", "minecraft:axolotl_bucket"));
    assert!(errors(&c).is_empty(), "{:#?}", errors(&c));
    let uses = delvec::compiler::bodies::owed_uses(&c);
    assert_eq!(uses.len(), 1, "{uses:#?}");
    assert_eq!(
        (uses[0].objective, uses[0].tool, uses[0].kind),
        ("obj/scoop", "minecraft:water_bucket", "minecraft:axolotl")
    );
    let out = build(&c);

    let act = function(&out, "activate_o_scoop").expect("the collect activates");
    assert!(
        act.lines().any(|l| l == "kill @e[tag=dw_abody_pip]"),
        "the box steps aside when the collect opens:\n{act}"
    );
    let tick = function(&out, "tick").unwrap();
    assert!(
        tick.lines()
            .any(|l| l.ends_with("run function hello-world:activate_o_scoop")),
        "the tick drives the activation"
    );

    let spawn = function(&out, "spawn_actor_pip").unwrap();
    let held = [
        "scoreboard players set #abody_aside dw.sys 0",
        "execute if score #act_o_scoop dw.sys matches 1 unless score #party dw.o_o_scoop matches 1 \
         run scoreboard players set #abody_aside dw.sys 1",
    ];
    for l in held {
        assert!(spawn.lines().any(|s| s == l), "{l}\n---\n{spawn}");
    }
    let summon = spawn
        .lines()
        .find(|l| summons_box(l))
        .expect("the box is summoned");
    assert!(
        summon.starts_with(
            "execute if score #abody_aside dw.sys matches 0 at @e[tag=dw_pup_pip,limit=1]"
        ),
        "a spawn inside the window never stands the box: {summon}"
    );

    let done = function(&out, "complete_o_scoop").unwrap();
    assert!(
        done.lines()
            .any(|l| summons_box(l) && l.starts_with("execute if score #abody_aside")),
        "the box returns over a body that still stands once the collect is done:\n{done}"
    );
    let order = |needle: &str| done.lines().position(|l| l.contains(needle)).unwrap();
    assert!(
        order("dw.o_o_scoop 1") < order("set #abody_aside dw.sys 0"),
        "the collect is done before the box asks whether it is open:\n{done}"
    );
}

/// The perturbation only the table can answer: the same body and the same
/// gathered collect, its item one no use on an axolotl yields. Nothing steps
/// aside, and the spawn's box line is the unheld summon it always was.
#[test]
fn a_gathered_item_no_use_on_the_body_yields_leaves_its_box_standing() {
    let c = campaign(&raw("minecraft:axolotl", "minecraft:cod_bucket"));
    assert!(errors(&c).is_empty(), "{:#?}", errors(&c));
    assert!(delvec::compiler::bodies::owed_uses(&c).is_empty());
    let out = build(&c);
    assert!(
        function(&out, "activate_o_scoop").is_none(),
        "a gathered collect owed no use places nothing and activates nothing"
    );
    let spawn = function(&out, "spawn_actor_pip").unwrap();
    assert!(!spawn.contains("#abody_aside"), "{spawn}");
    let summon = spawn.lines().find(|l| summons_box(l)).unwrap();
    assert!(
        summon.starts_with("execute at @e[tag=dw_pup_pip,limit=1]"),
        "{summon}"
    );
    let done = function(&out, "complete_o_scoop").unwrap();
    assert!(!done.contains("dw_abody_pip"), "{done}");
}

/// The quantifier is the kind: a cod's box steps aside for a gathered cod
/// bucket, as an axolotl's does for an axolotl bucket — the class is a body
/// with a use and a box, not one creature.
#[test]
fn every_bucketable_kind_is_the_object_of_its_own_bucket() {
    for (item, _, kind) in delvec::compiler::bodies::BODY_USES {
        let c = campaign(&raw(kind, item));
        let uses = delvec::compiler::bodies::owed_uses(&c);
        assert_eq!(uses.len(), 1, "{kind} / {item}: {uses:#?}");
        assert_eq!(uses[0].kind, *kind);
    }
}
