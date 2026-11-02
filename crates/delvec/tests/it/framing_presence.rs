//! spec-0110 criterion 7, build half: a body subject is present at the shot's
//! first tick (`DW1034` otherwise), judged by the one ordering rule the DSL's
//! `DW0195`/`DW0198` share (`crates/dsl/tests/npc_lifecycle_shared.rs`).

mod common;

use common::framing_fixture as ff;
use serde_json::{Value, json};

fn limb_shot() -> Value {
    ff::shot([-3, 2, 0], json!({"assembly": "assembly/limb"}))
}

fn talk(q: &mut Value) -> &mut Vec<Value> {
    q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .unwrap()
}

/// The fixture with the spawn removed from `obj/talk` and `place` deciding
/// where it and the cutscene go.
fn arranged(place: impl FnOnce(&mut Vec<Value>, Value, Value)) -> Value {
    common::assembly_fixture::quests_with(|q| {
        let b = talk(q);
        let spawn = b.pop().expect("the fixture's spawn-assembly is last");
        assert_eq!(spawn["type"], "spawn-assembly");
        place(b, spawn, ff::cutscene(vec![limb_shot()]));
    })
}

#[test]
fn a_spawn_ahead_in_the_same_list_is_present() {
    let out = ff::green(&ff::quests_with_cutscene(ff::cutscene(vec![limb_shot()])));
    let l = ff::ledger(&out);
    let p = l["cutscenes"][0]["shots"][0]["subjects"][0]["presence"]
        .as_str()
        .unwrap();
    assert!(
        p.contains("on_objective_complete/obj~1talk/1") || p.contains("obj/talk/1"),
        "{p}"
    );
}

#[test]
fn a_spawn_after_the_cutscene_is_dw1034() {
    let q = arranged(|b, spawn, cs| {
        b.push(cs);
        b.push(spawn);
    });
    let m = ff::refusal(&q, "DW1034");
    assert!(m.contains("no beat places before the cutscene"), "{m}");
    assert!(m.contains("spawn-assembly"), "{m}");
}

#[test]
fn a_spawn_in_a_later_sequence_step_is_dw1034() {
    let q = arranged(|b, spawn, cs| {
        b.push(json!({"type": "sequence", "steps": [
            {"at_ticks": 0, "effects": [cs]},
            {"at_ticks": 60, "effects": [spawn]}
        ]}));
    });
    ff::refusal(&q, "DW1034");
}

#[test]
fn a_spawn_behind_a_flag_gate_is_dw1034() {
    let q = arranged(|b, mut spawn, cs| {
        spawn["when"] = json!({"requires_flags": ["flag/struck"]});
        b.push(spawn);
        b.push(cs);
    });
    ff::refusal(&q, "DW1034");
}

#[test]
fn a_despawn_between_is_dw1034_naming_both() {
    let q = arranged(|b, spawn, cs| {
        b.push(spawn);
        b.push(json!({"type": "despawn-assembly", "assembly": "assembly/limb"}));
        b.push(cs);
    });
    let m = ff::refusal(&q, "DW1034");
    assert!(m.contains("placed by") && m.contains("removed by"), "{m}");
}

/// A cutscene does not hold its list: a despawn after it in the same list
/// runs at its first driver tick, so the shot frames an empty mark.
#[test]
fn a_despawn_after_the_cutscene_in_its_own_list_is_dw1034() {
    let q = arranged(|b, spawn, cs| {
        b.push(spawn);
        b.push(cs);
        b.push(json!({"type": "despawn-assembly", "assembly": "assembly/limb"}));
    });
    let m = ff::refusal(&q, "DW1034");
    assert!(
        m.contains("removed by") && m.contains("while the shot plays"),
        "{m}"
    );
    assert!(m.contains("`then`"), "{m}");
}

/// A later `sequence` step that runs before the shot ends removes the body
/// on screen; one that runs after the cutscene has ended does not.
#[test]
fn a_despawn_in_a_step_while_the_cutscene_plays_is_dw1034_and_after_it_is_present() {
    let during = arranged(|b, spawn, cs| {
        b.push(spawn);
        b.push(json!({"type": "sequence", "steps": [
            {"at_ticks": 0, "effects": [cs]},
            {"at_ticks": 20, "effects": [
                {"type": "despawn-assembly", "assembly": "assembly/limb"}]}
        ]}));
    });
    ff::refusal(&during, "DW1034");
    let after = arranged(|b, spawn, cs| {
        b.push(spawn);
        b.push(json!({"type": "sequence", "steps": [
            {"at_ticks": 0, "effects": [cs]},
            {"at_ticks": 200, "effects": [
                {"type": "despawn-assembly", "assembly": "assembly/limb"}]}
        ]}));
    });
    ff::green(&after);
}

/// The remedy the refusal names is reachable: the despawn in the cutscene's
/// own `then` runs when the camera returns, and the body is present.
#[test]
fn a_despawn_in_the_cutscenes_then_is_present() {
    let q = arranged(|b, spawn, mut cs| {
        cs["then"] = json!([{"type": "despawn-assembly", "assembly": "assembly/limb"}]);
        b.push(spawn);
        b.push(cs);
    });
    let out = ff::green(&q);
    let p = ff::ledger(&out)["cutscenes"][0]["shots"][0]["subjects"][0]["presence"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(p.starts_with("placed by") && !p.contains("removed"), "{p}");
}

#[test]
fn an_npc_placed_at_setup_is_present_with_no_beat() {
    let out = ff::green(&ff::quests_with_cutscene(ff::cutscene(vec![ff::shot(
        [-3, 2, -6],
        json!({"npc": "npc/keeper"}),
    )])));
    let l = ff::ledger(&out);
    assert_eq!(
        l["cutscenes"][0]["shots"][0]["subjects"][0]["presence"],
        "placed by setup"
    );
}

#[test]
fn an_actor_spawned_by_a_prerequisite_beat_is_present() {
    let q = common::assembly_fixture::quests_with(|q| {
        q["content"]["actors"] = json!([{
            "id": "actor/porter", "entity": "minecraft:villager",
            "anchor": "anchor/keeper-stand", "offset": [2, 0, 0]
        }]);
        talk(q).push(json!({"type": "spawn-actor", "actor": "actor/porter"}));
        q["content"]["quests"][0]["on_complete"]
            .as_array_mut()
            .unwrap()
            .insert(
                0,
                ff::cutscene(vec![ff::shot(
                    [-3, 2, -6],
                    json!({"actor": "actor/porter"}),
                )]),
            );
    });
    let out = ff::green(&q);
    let p = ff::ledger(&out)["cutscenes"][0]["shots"][0]["subjects"][0]["presence"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        p.starts_with("placed by /content/quests/0/on_objective_complete"),
        "{p}"
    );
}
