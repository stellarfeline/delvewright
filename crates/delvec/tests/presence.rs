//! **One named character is one body** (`DW1026`, `compiler::presence`).
//!
//! Each case is hello-world plus one edit that puts a second body carrying
//! the keeper's name somewhere in the story, and the assertion is about which
//! story state — if any — holds both at once. The perturbations vary one thing
//! each: whether the two lines share a list (one tick) or the despawn waits for a
//! later list, whether the second body is an NPC or an actor, whether the name is
//! the same once markup is dropped, and whether the spawn is a line the play
//! order can date at all.

mod common;

use delvewright_dsl::{Campaign, RawCampaign, parse_campaign};
use serde_json::{Value, json};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// hello-world with `npcs` and `quests` each patched by its closure.
fn campaign(npcs: impl FnOnce(&mut Value), quests: impl FnOnce(&mut Value)) -> Campaign {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: common::patch_doc(&read_hw("npcs.json"), npcs),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: common::patch_doc(&read_hw("quests.json"), quests),
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

/// A second NPC carrying `name`, deferred or on stage from world init.
fn second_npc(doc: &mut Value, name: &str, deferred: bool) {
    let mut n = doc["content"]["npcs"][0].clone();
    n["id"] = json!("npc/keeper-later");
    n["name"] = json!(name);
    if deferred {
        n["deferred"] = json!(true);
    }
    doc["content"]["npcs"].as_array_mut().unwrap().push(n);
}

fn line(kind: &str, field: &str, id: &str) -> Value {
    json!({ "type": kind, field: id, "happening": { "verb": "arrives", "text": format!("{kind} {id}") } })
}

fn talk_bundle(doc: &mut Value) -> &mut Vec<Value> {
    common::objective_effects(doc, 0, "obj/talk")
}

fn refusals(c: &Campaign) -> Vec<delvewright_dsl::Diagnostic> {
    let (d, _) = delvec::compiler::presence::check(c);
    d.into_iter().filter(|x| x.code == "DW1026").collect()
}

#[test]
fn hello_world_has_one_body_per_name() {
    let c = campaign(|_| {}, |_| {});
    let (d, bind) = delvec::compiler::presence::check(&c);
    assert!(d.is_empty(), "{d:#?}");
    assert_eq!(bind.npcs, 1);
    assert_eq!(bind.shared_names, 0);
    assert!(
        bind.configurations >= 1 && bind.states >= 1,
        "{}",
        bind.line()
    );
}

#[test]
fn two_npcs_with_one_name_on_stage_from_world_init_are_refused() {
    let c = campaign(|n| second_npc(n, "The Keeper", false), |_| {});
    let d = refusals(&c);
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].path, "/content/npcs/1/name");
    assert!(d[0].message.contains("`The Keeper`"), "{}", d[0].message);
    assert!(d[0].message.contains("from world init"), "{}", d[0].message);
}

#[test]
fn a_spawn_beside_no_despawn_is_refused_at_the_spawn() {
    let c = campaign(
        |n| second_npc(n, "The Keeper", true),
        |q| talk_bundle(q).push(line("spawn-npc", "npc", "npc/keeper-later")),
    );
    let d = refusals(&c);
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(
        d[0].path,
        "/content/quests/0/on_objective_complete/obj/talk/1"
    );
}

#[test]
fn a_despawn_in_the_same_list_hands_the_character_over() {
    // The order inside one list does not matter: both lines run in one tick.
    for despawn_first in [true, false] {
        let c = campaign(
            |n| second_npc(n, "The Keeper", true),
            |q| {
                let b = talk_bundle(q);
                let spawn = line("spawn-npc", "npc", "npc/keeper-later");
                let despawn = line("despawn-npc", "npc", "npc/keeper");
                if despawn_first {
                    b.push(despawn);
                    b.push(spawn);
                } else {
                    b.push(spawn);
                    b.push(despawn);
                }
            },
        );
        assert!(refusals(&c).is_empty(), "despawn_first={despawn_first}");
    }
}

#[test]
fn a_despawn_a_later_tick_leaves_both_bodies_standing_meanwhile() {
    let c = campaign(
        |n| second_npc(n, "The Keeper", true),
        |q| {
            let b = talk_bundle(q);
            b.push(line("spawn-npc", "npc", "npc/keeper-later"));
            b.push(json!({
                "type": "sequence",
                "steps": [{ "at_ticks": 40, "effects": [line("despawn-npc", "npc", "npc/keeper")] }]
            }));
        },
    );
    assert_eq!(refusals(&c).len(), 1);
    // …and the mirror: the old body leaves first, the new one arrives later.
    let c = campaign(
        |n| second_npc(n, "The Keeper", true),
        |q| {
            let b = talk_bundle(q);
            b.push(line("despawn-npc", "npc", "npc/keeper"));
            b.push(json!({
                "type": "sequence",
                "steps": [{ "at_ticks": 40, "effects": [line("spawn-npc", "npc", "npc/keeper-later")] }]
            }));
        },
    );
    assert!(refusals(&c).is_empty());
}

#[test]
fn another_name_is_another_person() {
    let c = campaign(
        |n| second_npc(n, "The Keeper's Brother", true),
        |q| talk_bundle(q).push(line("spawn-npc", "npc", "npc/keeper-later")),
    );
    assert!(refusals(&c).is_empty());
}

#[test]
fn a_name_is_compared_as_it_is_drawn() {
    let c = campaign(|n| second_npc(n, "[[bold|The Keeper]]", false), |_| {});
    assert_eq!(refusals(&c).len(), 1);
}

#[test]
fn an_actor_wearing_the_name_is_a_second_body() {
    let actor = |q: &mut Value| {
        q["content"]["actors"] = json!([{
            "id": "actor/keeper-shade",
            "entity": "minecraft:villager",
            "name": "The Keeper",
            "anchor": "anchor/exit"
        }]);
    };
    let c = campaign(
        |_| {},
        |q| {
            actor(q);
            talk_bundle(q).push(line("spawn-actor", "actor", "actor/keeper-shade"));
        },
    );
    let d = refusals(&c);
    assert_eq!(d.len(), 1, "{d:#?}");
    assert!(d[0].message.contains("`actor/keeper-shade`"));
    let c = campaign(
        |_| {},
        |q| {
            actor(q);
            let b = talk_bundle(q);
            b.push(line("despawn-npc", "npc", "npc/keeper"));
            b.push(line("spawn-actor", "actor", "actor/keeper-shade"));
        },
    );
    assert!(refusals(&c).is_empty());
}

#[test]
fn a_spawn_the_play_order_cannot_date_can_happen_in_any_state() {
    let c = campaign(
        |n| second_npc(n, "The Keeper", true),
        |q| {
            q["content"]["triggers"] = json!([{
                "id": "trigger/keeper-returns",
                "at": "anchor/exit",
                "on": { "on": "approach", "range": 2 },
                "effects": [line("spawn-npc", "npc", "npc/keeper-later")]
            }]);
        },
    );
    let (d, bind) = delvec::compiler::presence::check(&c);
    assert_eq!(bind.undatable, 1, "{}", bind.line());
    assert_eq!(d.iter().filter(|x| x.code == "DW1026").count(), 1, "{d:#?}");
    assert!(d[0].message.contains("cannot date"), "{}", d[0].message);
}

#[test]
fn validate_prints_the_binding() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args([
            "validate",
            common::hello_world_dir().to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let text =
        String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    assert!(
        text.contains("presence binding (DW1026): 1 npc(s)"),
        "{text}"
    );
}

/// A realised branch is walked in the story world that realises it (spec-0108
/// §4.2): `RealizedBranch::world` indexes the story worlds, which outnumber
/// the dialogue-choice worlds once a campaign has an elective act. On
/// `tests/fixtures/elective-reach` a branch is realised by a story world past
/// the last choice world, so reading its index as a choice world's is out of
/// bounds; the check walks every configuration instead.
#[test]
fn a_branch_is_walked_in_the_story_world_that_realises_it() {
    let dir = common::compiler_fixtures_dir().join("elective-reach");
    let read = |f: &str| std::fs::read_to_string(dir.join(f)).unwrap();
    let c = parse_campaign(&RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: read("quests.json"),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("fixture parses");
    let flow = delvec::compiler::flow::Flow::new(&c);
    let realized = delvec::compiler::branch::realize(&c);
    let worlds: Vec<usize> = realized.iter().filter_map(|r| r.world).collect();
    assert!(
        worlds.iter().any(|&w| w >= flow.world_count()),
        "the fixture must realise a branch past the choice worlds: story worlds {worlds:?}, \
         {} choice world(s)",
        flow.world_count()
    );
    let (d, bind) = delvec::compiler::presence::check(&c);
    assert_eq!(bind.configurations, 1 + worlds.len(), "{}", bind.line());
    assert!(d.iter().all(|x| x.code != "DW1026"), "{d:#?}");
}
