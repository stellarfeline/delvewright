//! spec-0132 §4 — the entrance registry's refusals: `DW1081` (an entrance the
//! kind does not have), `DW1082` (a declaration about the free body on an actor
//! nothing frees; the build-time arm is the compiler's), `DW1083` (a puppet
//! surface on an actor that enters), and the ambush sugar that gives an
//! entering actor its unleash alone.
//!
//! Built on the hello-world campaign with a stage-5 document carrying one
//! warden that digs out of the ground when `obj/talk` completes. Each case
//! perturbs that green campaign by one edit and states the codes it raises.

use crate::common;

use std::collections::BTreeSet;

use delvewright_dsl::{RawCampaign, Verb, check_campaign, parse_campaign};
use serde_json::{Value, json};

const SLEEPER: &str = "actor/sleeper";

/// The green campaign's stage-5 document, as JSON to perturb.
fn quests() -> Value {
    json!({
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "quests",
        "content": {
            "quests": [{
                "id": "quest/open-the-door",
                "trigger": { "type": "campaign-start" },
                "objectives": [
                    { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
                    { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit", "radius": 2, "after": ["obj/talk"] }
                ],
                "on_objective_complete": {
                    "obj/talk": [
                        { "type": "open-gate", "anchor": "anchor/door" },
                        { "type": "unleash-actor", "actor": SLEEPER }
                    ]
                },
                "on_complete": [ { "type": "campaign-complete" } ]
            }],
            "actors": [
                { "id": SLEEPER, "entity": "minecraft:warden", "anchor": "anchor/keeper-stand",
                  "facing": "north", "entrance": "native", "outrun": true }
            ]
        }
    })
}

fn raw(q: &Value) -> RawCampaign {
    RawCampaign {
        world: common::read_valid("world.json").replacen(
            "\"target_minutes\": 5,",
            "\"target_minutes\": 5,\n    \"difficulty\": \"normal\",",
            1,
        ),
        npcs: common::read_valid("npcs.json"),
        classes: common::read_valid("classes.json"),
        quest_plan: common::read_valid("quest-plan.json"),
        quests: serde_json::to_string(q).unwrap(),
        dialogue: common::read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

/// The spec-0132 codes a campaign raises, each with its path.
fn entrance_codes(q: &Value) -> BTreeSet<(String, String)> {
    check_campaign(&raw(q))
        .into_iter()
        .filter(|d| matches!(d.code.as_str(), "DW1081" | "DW1082" | "DW1083"))
        .map(|d| (d.code, d.path))
        .collect()
}

fn actor(q: &mut Value) -> &mut Value {
    &mut q["content"]["actors"][0]
}

fn talk(q: &mut Value) -> &mut Vec<Value> {
    q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .unwrap()
}

fn set(code: &str, path: &str) -> BTreeSet<(String, String)> {
    [(code.to_string(), path.to_string())].into_iter().collect()
}

#[test]
fn the_green_campaign_raises_none_of_the_three() {
    let diags = check_campaign(&raw(&quests()));
    assert!(
        !diags
            .iter()
            .any(|d| matches!(d.code.as_str(), "DW1081" | "DW1082" | "DW1083")),
        "{diags:#?}"
    );
}

#[test]
fn dw1081_an_entrance_on_a_kind_that_has_none() {
    let mut q = quests();
    actor(&mut q)["entity"] = json!("minecraft:zombie");
    assert_eq!(
        entrance_codes(&q),
        set("DW1081", "/content/actors/0/entrance")
    );
    let d = check_campaign(&raw(&q))
        .into_iter()
        .find(|d| d.code == "DW1081")
        .unwrap();
    for named in [
        SLEEPER,
        "minecraft:zombie",
        "`enter`",
        "`minecraft:warden`",
        "`minecraft:wither`",
    ] {
        assert!(d.message.contains(named), "`{named}` named: {}", d.message);
    }
}

#[test]
fn dw1082_an_entrance_nothing_unleashes() {
    let mut q = quests();
    actor(&mut q)["outrun"] = json!(false);
    talk(&mut q).retain(|e| e["type"] != "unleash-actor");
    assert_eq!(
        entrance_codes(&q),
        set("DW1082", "/content/actors/0/entrance")
    );
}

#[test]
fn dw1082_an_outrun_nothing_unleashes() {
    let mut q = quests();
    actor(&mut q).as_object_mut().unwrap().remove("entrance");
    talk(&mut q).retain(|e| e["type"] != "unleash-actor");
    assert_eq!(
        entrance_codes(&q),
        set("DW1082", "/content/actors/0/outrun")
    );
}

#[test]
fn dw1083_every_puppet_surface_on_an_entering_actor() {
    let cases: Vec<(&str, Box<dyn Fn(&mut Value)>, &str)> = vec![
        (
            "a spawn-actor",
            Box::new(|q| talk(q).insert(0, json!({ "type": "spawn-actor", "actor": SLEEPER }))),
            "/content/quests/0/on_objective_complete/obj/talk/0",
        ),
        (
            "a move-actor",
            Box::new(|q| {
                talk(q).push(json!({ "type": "move-actor", "actor": SLEEPER, "to": { "anchor": "anchor/exit" } }))
            }),
            "/content/quests/0/on_objective_complete/obj/talk/2",
        ),
        (
            "a hand-written ambush spawn",
            Box::new(|q| {
                q["content"]["triggers"] = json!([{
                    "id": "trigger/the-floor",
                    "at": "anchor/exit",
                    "on": { "on": "approach", "range": 3 },
                    "once": true,
                    "effects": [
                        { "type": "spawn-actor", "actor": SLEEPER },
                        { "type": "unleash-actor", "actor": SLEEPER }
                    ]
                }]);
            }),
            "/content/triggers/0/effects/0",
        ),
        (
            "vulnerable",
            Box::new(|q| actor(q)["vulnerable"] = json!(true)),
            "/content/actors/0/vulnerable",
        ),
        (
            "watch",
            Box::new(|q| actor(q)["watch"] = json!({ "who": "nearest", "within": 4 })),
            "/content/actors/0/watch",
        ),
        (
            "answers",
            Box::new(|q| actor(q)["answers"] = json!({ "struck": "Stone.", "touched": "Cold." })),
            "/content/actors/0/answers",
        ),
    ];
    for (what, edit, path) in cases {
        let mut q = quests();
        edit(&mut q);
        let got: BTreeSet<(String, String)> = entrance_codes(&q)
            .into_iter()
            .filter(|(c, _)| c == "DW1083")
            .collect();
        assert_eq!(got, set("DW1083", path), "{what}");
    }
}

/// Criterion 7: the ambush sugar desugars an ambush over one entering and one
/// plain actor to one `spawn-actor` and two `unleash-actor`s, in that order —
/// and the entering actor's would-be spawn is never written, so no `DW1083`.
#[test]
fn the_ambush_sugar_omits_the_spawn_for_an_entering_actor() {
    let mut q = quests();
    talk(&mut q).retain(|e| e["type"] != "unleash-actor");
    q["content"]["actors"].as_array_mut().unwrap().push(json!({
        "id": "actor/guard", "entity": "minecraft:zombie", "anchor": "anchor/exit", "facing": "south"
    }));
    q["content"]["ambushes"] = json!([{
        "id": "ambush/the-floor",
        "at": "anchor/exit",
        "actors": [SLEEPER, "actor/guard"],
        "trigger": { "on": "approach", "range": 3 }
    }]);
    let c = parse_campaign(&raw(&q)).expect("parses");
    let t = c
        .quests
        .content
        .triggers
        .iter()
        .find(|t| t.id.as_str() == "trigger/the-floor")
        .expect("the ambush desugars");
    let shape: Vec<String> = t
        .effects
        .iter()
        .map(|e| match &e.verb {
            Verb::SpawnActor { actor } => format!("spawn {actor}"),
            Verb::UnleashActor { actor } => format!("unleash {actor}"),
            v => panic!("unexpected {v:?}"),
        })
        .collect();
    assert_eq!(
        shape,
        vec![
            "spawn actor/guard".to_string(),
            format!("unleash {SLEEPER}"),
            "unleash actor/guard".to_string(),
        ]
    );
    assert!(
        !entrance_codes(&q).iter().any(|(c, _)| c == "DW1083"),
        "the sugar writes no spawn for the entering actor"
    );
}
