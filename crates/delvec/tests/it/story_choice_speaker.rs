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
/// any other), and `shy` the flag that withdraws his option, if any.
fn fixture(tag: &str, gate: Option<&str>, shy: Option<&str>) -> PathBuf {
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
                            "forbids_flags": shy.into_iter().collect::<Vec<_>>(),
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

/// A choice the selection owes is taken in front of the LAST step at which
/// it can be taken, the entry deeds' rule: the town's arrival in Magnhild's
/// cellar was a reach armed in front of her, and a choice taken in front of
/// it walked into its volume (`DW0311`).
#[test]
fn a_choice_is_taken_as_late_as_the_reading_allows() {
    let (path, choices) = trusted_choices(&parse(&fixture("open", None, None)));
    // Binding: the path walks the keeper, then the door, and the trusted world
    // takes the hermit's choice once.
    assert_eq!(path, ["obj/talk", "obj/exit"], "{path:?}");
    assert_eq!(
        choices,
        [("npc/hermit".to_string(), "obj/exit".to_string())],
        "the option is offered in front of both steps, so it is taken in front of the last"
    );
}

/// A choice is never taken from behind a way that is shut where it stands.
/// The hermit's den opens on `obj/talk`, and his option is withdrawn by the
/// same flag: it is offered only while the den is shut, so no point of the
/// path can take it — before, the trusted world took it in front of
/// `obj/talk` and walked from the spawn into a shut den.
#[test]
fn a_choice_is_never_taken_from_behind_a_shut_way() {
    let (path, choices) = trusted_choices(&parse(&fixture(
        "shy",
        Some("flag/den-open"),
        Some("flag/den-open"),
    )));
    assert_eq!(path, ["obj/talk", "obj/exit"], "{path:?}");
    assert!(choices.is_empty(), "{choices:?}");
}

/// **A gate on the way to a completing option is a read** of the step that
/// takes it. The keeper's completing option moves behind his lore, and the
/// option that opens the lore asks `flag/trusted`, the flag the hermit's
/// choice sets: the trusted world must take the choice in front of
/// `obj/talk`. Read by the completing option's own gate alone, the walk saw
/// no read there and took the choice last, and the replay refused the path at
/// `obj/talk` (the town's `obj/hear-magnhild`, behind Magnhild's founder
/// question, `DW0204`).
#[test]
fn a_gate_on_the_way_to_the_completing_option_is_paid_in_front_of_the_step() {
    let dir = fixture("way", None, None);
    edit(&dir.join("dialogue.json"), |v| {
        let keeper = &mut v["content"]["dialogues"][0]["nodes"];
        let open = keeper[0]["options"].as_array_mut().unwrap().remove(1);
        keeper[0]["options"][0]["requires_flags"] = json!(["flag/trusted"]);
        keeper[1]["options"].as_array_mut().unwrap().push(open);
    });
    let c = parse(&dir);
    let (path, choices) = trusted_choices(&c);
    assert_eq!(path, ["obj/talk", "obj/exit"], "{path:?}");
    assert_eq!(
        choices,
        [("npc/hermit".to_string(), "obj/talk".to_string())],
        "the lore's gate is read at `obj/talk`, so the choice is paid in front of it"
    );
    let flow = Flow::new(&c);
    let r = branch::realize(&c)
        .into_iter()
        .find(|r| r.branch.id == "branch/trusted")
        .expect("the trusted branch is enumerated");
    let pt = flow.story_playthrough(r.world.expect("a story world realizes it"));
    assert!(flow.replay(&pt).is_ok(), "the trusted world's path replays");
}

/// A speaker no way ever reaches offers nothing: the solve does not credit
/// the hermit's flag, so no story world's path takes his choice — before, the
/// worlds whose selection took it walked from the spawn to a den that never
/// opens.
#[test]
fn a_speaker_no_way_reaches_offers_nothing() {
    let c = parse(&fixture("sealed", Some("flag/never-set"), None));
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

/// hello-world whose keeper is reached by an arrival armed from the start
/// (`obj/arrive`, a reach at his stand), with a fork on an option of his
/// lore that completes nothing: the trusted world owes that choice.
fn arrival_fixture() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("story-choice-arrival");
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    edit(&dir.join("dialogue.json"), |v| {
        v["content"]["dialogues"][0]["nodes"][1]["options"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "label": "Tell me a secret.",
                "effects": [{"type": "set-flag", "flag": "flag/trusted"}],
                "happening": {"text": "the keeper trusts the party", "verb": "believes"}
            }));
    });
    edit(&dir.join("quests.json"), |v| {
        let objs = v["content"]["quests"][0]["objectives"]
            .as_array_mut()
            .unwrap();
        objs[0]["after"] = json!(["obj/arrive"]);
        objs.insert(
            0,
            json!({
                "id": "obj/arrive", "type": "reach-anchor", "anchor": "anchor/keeper-stand",
                "radius": 2, "title": "Come to the keeper", "hint": "He keeps the gate room.",
                "happening": {"text": "the party completes obj/arrive", "verb": "arrives"}
            }),
        );
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
    dir
}

/// **The town's shape, built**: a choice taken in front of the arrival would
/// walk from the spawn into the arrival's volume while it is armed and not yet
/// due (`DW0311`); taken as late as the reading allows, it comes after the
/// arrival and the build proves every world's walk.
#[test]
fn a_choice_behind_an_armed_arrival_builds() {
    let dir = arrival_fixture();
    let out = dir.with_extension("out");
    let _ = std::fs::remove_dir_all(&out);
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args([
            "build",
            dir.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--prefabs",
            common::prefabs_dir().to_str().unwrap(),
        ])
        .output()
        .expect("delvec runs");
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(o.status.code(), Some(0), "{log}");
    // Binding: some world's exported path takes the keeper's choice, after
    // the arrival.
    let mut chose = 0;
    for f in std::fs::read_dir(out.join("validation")).unwrap() {
        let f = f.unwrap().path();
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        if !name.starts_with("world-path-") {
            continue;
        }
        let path: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        let steps = path["steps"].as_array().unwrap();
        let at = |pred: &dyn Fn(&Value) -> bool| steps.iter().position(pred);
        if let Some(c) = at(&|s| s["action"] == "choose") {
            let arrive = at(&|s| s["objective"] == "obj/arrive").expect("the arrival is walked");
            assert!(arrive < c, "{name}: the choice comes after the arrival");
            chose += 1;
        }
    }
    assert!(chose > 0, "no world took the keeper's choice");
}
