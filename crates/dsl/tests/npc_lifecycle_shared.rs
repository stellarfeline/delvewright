//! spec-0110 criterion 7, DSL half: a body's lifetime is judged by ONE
//! ordering rule, `ordering::Order::ordered_before`, which `DW0195`, `DW0198`
//! and the build's `DW1034` (through `Order::presence`) all call.

mod common;

use delvewright_dsl::ordering::{Beat, Order, Presence};
use delvewright_dsl::{Campaign, RawCampaign, Verb, parse_campaign};
use serde_json::{Value, json};

/// The hello-world campaign with `edit` applied to its quests document.
fn campaign(edit: impl FnOnce(&mut Value)) -> Campaign {
    let mut q: Value = serde_json::from_str(&common::read_valid("quests.json")).unwrap();
    edit(&mut q);
    let raw = RawCampaign {
        quests: q.to_string(),
        ..common::valid_raw()
    };
    parse_campaign(&raw).expect("parses")
}

fn cutscene() -> Value {
    json!({"type": "cutscene", "seconds": 2, "subject": {"assembly": "assembly/x"},
           "path": [{"anchor": "anchor/exit", "offset": [0, 2, 0]}]})
}

fn spawn() -> Value {
    json!({"type": "spawn-assembly", "assembly": "assembly/x"})
}

/// The one `cutscene` effect of `c`.
fn the_cutscene(c: &Campaign) -> &delvewright_dsl::QuestEffect {
    let mut found = None;
    delvewright_dsl::for_each_campaign_effect(c, &mut |_p, _s, e| {
        if matches!(e.verb, Verb::Cutscene { .. }) {
            found = Some(e);
        }
    });
    found.expect("one cutscene")
}

fn presence(edit: impl FnOnce(&mut Value), body: &str) -> Presence {
    let c = campaign(edit);
    let order = Order::of(&c);
    let p = order.presence(body, Beat::Effect(the_cutscene(&c)));
    p.clone()
}

fn talk_bundle(q: &mut Value) -> &mut Vec<Value> {
    q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .unwrap()
}

#[test]
fn a_spawn_ahead_in_the_same_list_is_present() {
    let p = presence(
        |q| {
            let b = talk_bundle(q);
            b.push(spawn());
            b.push(cutscene());
        },
        "assembly/x",
    );
    assert!(matches!(p, Presence::Present(_)), "{p:?}");
}

#[test]
fn a_spawn_after_the_cutscene_is_never_placed() {
    let p = presence(
        |q| {
            let b = talk_bundle(q);
            b.push(cutscene());
            b.push(spawn());
        },
        "assembly/x",
    );
    assert!(
        matches!(p, Presence::NeverPlaced(ref f) if f.len() == 1),
        "{p:?}"
    );
}

#[test]
fn a_spawn_in_a_later_sequence_step_is_never_placed() {
    let p = presence(
        |q| {
            talk_bundle(q).push(json!({"type": "sequence", "steps": [
                {"at_ticks": 0, "effects": [cutscene()]},
                {"at_ticks": 20, "effects": [spawn()]}
            ]}));
        },
        "assembly/x",
    );
    assert!(matches!(p, Presence::NeverPlaced(_)), "{p:?}");
    // The other way round it is present.
    let p = presence(
        |q| {
            talk_bundle(q).push(json!({"type": "sequence", "steps": [
                {"at_ticks": 0, "effects": [spawn()]},
                {"at_ticks": 20, "effects": [cutscene()]}
            ]}));
        },
        "assembly/x",
    );
    assert!(matches!(p, Presence::Present(_)), "{p:?}");
}

#[test]
fn a_spawn_behind_a_flag_gate_counts_for_nothing() {
    let p = presence(
        |q| {
            let b = talk_bundle(q);
            let mut s = spawn();
            s["when"] = json!({"requires_flags": ["flag/x"]});
            b.push(s);
            b.push(cutscene());
        },
        "assembly/x",
    );
    assert!(matches!(p, Presence::NeverPlaced(_)), "{p:?}");
}

#[test]
fn a_despawn_between_the_spawn_and_the_cutscene_removes_it() {
    let p = presence(
        |q| {
            let b = talk_bundle(q);
            b.push(spawn());
            b.push(json!({"type": "despawn-assembly", "assembly": "assembly/x"}));
            b.push(cutscene());
        },
        "assembly/x",
    );
    assert!(matches!(p, Presence::Removed(_, _)), "{p:?}");
}

#[test]
fn an_npc_placed_at_setup_is_present_with_no_beat() {
    let p = presence(|q| talk_bundle(q).push(cutscene()), "npc/keeper");
    assert_eq!(p, Presence::Present("setup".to_string()));
}

#[test]
fn a_spawn_in_a_prerequisite_objectives_bundle_is_ordered_before_the_quests_on_complete() {
    let p = presence(
        |q| {
            talk_bundle(q).push(json!({"type": "spawn-actor", "actor": "actor/x"}));
            q["content"]["quests"][0]["on_complete"]
                .as_array_mut()
                .unwrap()
                .insert(0, cutscene());
        },
        "actor/x",
    );
    assert!(matches!(p, Presence::Present(_)), "{p:?}");
}

/// The rule has one home: one definition, and `DW0195`, `DW0198` and the
/// build's presence proof all reach it.
#[test]
fn the_ordering_function_has_one_definition_and_three_callers() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut defs = Vec::new();
    let mut walk = vec![root.join("dsl/src"), root.join("delvec/src")];
    while let Some(dir) = walk.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let text = std::fs::read_to_string(&p).unwrap();
                for (i, l) in text.lines().enumerate() {
                    if l.contains("fn ordered_before") {
                        defs.push(format!("{}:{}", p.display(), i + 1));
                    }
                }
            }
        }
    }
    assert_eq!(defs.len(), 1, "{defs:?}");
    let npc = std::fs::read_to_string(root.join("dsl/src/npc.rs")).unwrap();
    let dw0195 = npc.split("fn despawned_ref_check").nth(1).unwrap();
    let dw0195 = dw0195.split("\npub(crate) fn ").next().unwrap();
    assert!(dw0195.contains("ordered_before"), "DW0195 calls the rule");
    let dw0198 = npc.split("fn deferred_npc_checks").nth(1).unwrap();
    let dw0198 = dw0198.split("\npub(crate) fn ").next().unwrap();
    assert!(dw0198.contains("ordered_before"), "DW0198 calls the rule");
    let framing = std::fs::read_to_string(root.join("delvec/src/compiler/framing.rs")).unwrap();
    assert!(
        framing.contains("order.presence("),
        "DW1034 calls it through presence"
    );
    let ordering = std::fs::read_to_string(root.join("dsl/src/ordering.rs")).unwrap();
    let pres = ordering.split("pub fn presence").nth(1).unwrap();
    assert!(
        pres.contains("self.ordered_before("),
        "presence is built on the rule"
    );
}
