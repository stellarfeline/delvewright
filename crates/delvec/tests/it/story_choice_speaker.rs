//! A story world's choice is taken from a speaker the party can walk to
//! (`Flow::speaker_reachable`).
//!
//! The fixture is hello-world with a second speaker, a hermit, stationed in a
//! den below a way that opens only once the keeper is heard (`obj/talk` sets
//! `flag/den-open`, the gating of the only connection into the den). The
//! hermit offers an option that completes no objective and sets
//! `flag/trusted`, and a fork on that flag makes it a choice the trusted
//! world's selection takes.
//!
//! The shape is the town's side story under a hidden floor: the story world
//! took the choice in front of its first step, where the option was offered,
//! and the route proof then walked from the spawn to a speaker no way reached
//! yet (`DW0317`, its gate counterfactual naming no seal). The choice now
//! waits for the step in front of which the den is open.

use crate::common;

use std::path::{Path, PathBuf};

use delvec::compiler::branch;
use delvec::compiler::flow::Flow;
use delvec::compiler::load::load_campaign_dir;
use delvewright_dsl::{Campaign, parse_campaign};
use serde_json::{Value, json};

fn edit(path: &Path, f: impl FnOnce(&mut Value)) {
    let mut v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    f(&mut v);
    std::fs::write(path, v.to_string()).unwrap();
}

/// hello-world with the hermit in the den; `gate` is the flag the den's one
/// connection waits on, if any (`obj/talk` sets `flag/den-open`; nothing sets
/// any other).
fn fixture(tag: &str, gate: Option<&str>) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("story-choice-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    edit(&dir.join("npcs.json"), |v| {
        let keeper = v["content"]["npcs"][0].clone();
        let mut hermit = keeper;
        hermit["id"] = json!("npc/hermit");
        hermit["name"] = json!("The Hermit");
        hermit["anchor"] = json!("anchor/hermit-stand");
        hermit["role"] = json!("flavor");
        v["content"]["npcs"].as_array_mut().unwrap().push(hermit);
    });
    edit(&dir.join("dialogue.json"), |v| {
        v["content"]["dialogues"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "npc": "npc/hermit",
                "root": "dlg/hermit",
                "nodes": [{
                    "id": "dlg/hermit",
                    "text": "Few find the den.",
                    "options": [
                        {
                            "label": "Tell me a secret.",
                            "effects": [{"type": "set-flag", "flag": "flag/trusted"}],
                            "happening": {"text": "the hermit trusts the party", "verb": "believes"}
                        },
                        {"label": "Goodbye."}
                    ]
                }]
            }));
    });
    edit(&dir.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        q["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "set-flag", "flag": "flag/den-open"}));
    });
    edit(&dir.join("quest-plan.json"), |v| {
        v["content"]["branch_points"] = json!([{
            "id": "branch-point/trust", "forks_on": ["flag/trusted"],
            "opens_at": "quest/open-the-door",
            "branches": [
                {"id": "branch/trusted", "flags": ["flag/trusted"],
                 "leads_to": "quest/open-the-door"},
                {"id": "branch/doubted", "flags": [], "leads_to": "quest/open-the-door"}
            ]
        }]);
    });
    let mut den = json!({"a": "node/yard", "b": "node/den", "class": "climb", "id": "edge/den"});
    if let Some(flag) = gate {
        den["gating"] = json!({"flags": [flag], "learned": "hidden", "secret": ["den"]});
    }
    let graph = json!({
        "campaign_id": "hello-world",
        "stage": "layout-graph",
        "dsl_version": "0.89.0",
        "content": {
            "entry": "node/yard",
            "goal": "node/yard",
            "critical_path": ["node/yard"],
            "nodes": [
                {"id": "node/yard", "intent": "arrival"},
                {"id": "node/den", "intent": "hideaway",
                 "stations": [{"anchor": "anchor/hermit-stand", "kind": "point"}]}
            ],
            "edges": [den]
        }
    });
    std::fs::write(dir.join("layout-graph.json"), graph.to_string()).unwrap();
    dir
}

fn parse(dir: &Path) -> Campaign {
    let loaded = load_campaign_dir(dir).expect("fixture loads");
    parse_campaign(&loaded.raw).expect("fixture parses")
}

/// The trusted world's path, by objective, and the step each choice it takes
/// stands in front of, as `(npc, objective)`.
fn trusted_choices(c: &Campaign) -> (Vec<String>, Vec<(String, String)>) {
    let flow = Flow::new(c);
    let r = branch::realize(c)
        .into_iter()
        .find(|r| r.branch.id == "branch/trusted")
        .expect("the trusted branch is enumerated");
    let pt = flow.story_playthrough(r.world.expect("a story world realizes it"));
    let ctx = pt.story.clone().expect("a story world's playthrough");
    let path: Vec<String> = pt.steps.iter().map(|s| s.objective.clone()).collect();
    let choices = ctx
        .choices
        .iter()
        .map(|(i, ch)| (ch.npc.clone(), path[*i].clone()))
        .collect();
    (path, choices)
}

#[test]
fn a_choice_waits_for_the_way_to_its_speaker() {
    let (path, choices) = trusted_choices(&parse(&fixture("gated", Some("flag/den-open"))));
    // Binding: the path walks the keeper, then the door, and the trusted world
    // takes the hermit's choice once.
    assert_eq!(path, ["obj/talk", "obj/exit"], "{path:?}");
    assert_eq!(
        choices,
        [("npc/hermit".to_string(), "obj/exit".to_string())],
        "the den opens on `obj/talk`, so the hermit is spoken to in front of the step after it"
    );
}

#[test]
fn a_choice_from_a_speaker_on_an_open_way_is_taken_where_it_is_offered() {
    let (_, choices) = trusted_choices(&parse(&fixture("open", None)));
    assert_eq!(
        choices,
        [("npc/hermit".to_string(), "obj/talk".to_string())],
        "nothing shuts the den, so the choice stands in front of the first step"
    );
}

/// A speaker no way ever reaches offers nothing: the solve does not credit
/// the hermit's flag, so no story world's path takes his choice — before, the
/// worlds whose selection took it walked from the spawn to a den that never
/// opens.
#[test]
fn a_speaker_no_way_reaches_offers_nothing() {
    let c = parse(&fixture("sealed", Some("flag/never-set")));
    let flow = Flow::new(&c);
    // Binding: the story worlds exist, so their paths were asked.
    let n = flow.story_count();
    assert!(n > 0, "the fixture has story worlds");
    let taken: Vec<usize> = (0..n)
        .filter(|&k| {
            flow.story_playthrough(k)
                .story
                .is_some_and(|ctx| ctx.choices.iter().any(|(_, ch)| ch.npc == "npc/hermit"))
        })
        .collect();
    assert!(
        taken.is_empty(),
        "worlds {taken:?} of {n} speak to the hermit"
    );
    // And no world owes a choice its path cannot take: the solve does not
    // credit the hermit's flag to a world that never opens the den, so the
    // trusted selection's choice is not owed there (else `DW0204`, "no point
    // of its path offers that option").
    let unpaid: Vec<&String> = flow.story().runs.iter().flat_map(|r| &r.unpaid).collect();
    assert!(unpaid.is_empty(), "{unpaid:#?}");
}
