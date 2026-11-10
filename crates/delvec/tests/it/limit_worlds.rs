//! spec-0133 §5 — the expired world. A limited objective's expiry is an
//! outcome the story worlds enumerate: one expiry witness per limit, the all
//! world with that one objective expired, exported as a world path whose step
//! for the objective is an `expire` — a wait, not an act — and in which the
//! objective's completion bundle credits nothing and its `on_expire` credits
//! at the objective's own step.
//!
//! The fixture is hello-world with a minute on the door and two endings: the
//! party home in time, or late. The late ending's scene reads the flag the
//! expiry writes, so the expiry witness is also the late ending's witness.

use crate::common;

use std::path::{Path, PathBuf};

use delvec::compiler::flow::{Flow, WorldOrigin};
use delvewright_dsl::parse_campaign;
use serde_json::{Value, json};

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-limit-worlds-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn ending(id: &str, when: Value) -> Value {
    json!({"type": "campaign-complete", "ending": id, "when": when,
           "happening": {"text": format!("{id}."), "verb": "survives"}})
}

fn late_flag() -> Value {
    json!({"type": "set-flag", "flag": "flag/late",
           "happening": {"text": "the party came too late", "verb": "learns"}})
}

/// hello-world with `limits` (objective index → limit) and two endings on the
/// expiry flag, the fork declared.
fn fixture(tag: &str, patch: impl FnOnce(&mut Value)) -> PathBuf {
    let dir = scratch(tag);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    common::patch_file(&dir.join("quest-plan.json"), |v| {
        v["content"]["branch_points"] = json!([{
            "id": "branch-point/late", "forks_on": ["flag/late"],
            "opens_at": "quest/open-the-door",
            "branches": [
                {"id": "branch/late", "flags": ["flag/late"], "leads_to": "quest/open-the-door"},
                {"id": "branch/in-time", "flags": [], "leads_to": "quest/open-the-door"}
            ]
        }]);
    });
    common::patch_file(&dir.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        q["objectives"][1]["limit"] = json!({
            "ticks": 1200,
            "on_expire": [
                {"type": "narrate", "text": "The keeper bars the door again.",
                 "happening": {"text": "the door shuts", "verb": "seals"}},
                late_flag()
            ]
        });
        q["on_complete"] = json!([
            ending("ending/late", json!({"requires_flags": ["flag/late"]})),
            ending("ending/home", json!({"forbids_flags": ["flag/late"]}))
        ]);
        patch(v);
    });
    dir
}

fn build(dir: &Path) -> (i32, String, PathBuf) {
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
    (
        o.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
        out,
    )
}

fn read(out: &Path, rel: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(out.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}")),
    )
    .unwrap()
}

fn campaign(dir: &Path) -> delvewright_dsl::Campaign {
    let loaded = delvec::compiler::load::load_campaign_dir(dir).unwrap();
    parse_campaign(&loaded.raw).expect("fixture parses")
}

/// The ending ids a world's row in the branch plan fires.
fn endings_of(plan: &Value, k: usize) -> Vec<String> {
    plan["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["id"] == format!("world/{k}"))
        .map(|b| {
            b["endings"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| e.as_str().unwrap().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn one_limit_is_the_all_world_and_one_expiry_witness() {
    let dir = fixture("one", |_| {});
    let c = campaign(&dir);
    let flow = Flow::new(&c);
    let story = flow.story();
    assert_eq!(story.worlds.len(), 2, "{:?}", story.worlds);
    assert_eq!(story.worlds[0].origin, WorldOrigin::All);
    assert!(
        story.worlds[0].performs.is_empty(),
        "the all world expires nothing"
    );
    let w = &story.worlds[1];
    assert_eq!(w.origin, WorldOrigin::Expiry);
    assert!(w.performs.contains("obj/exit/expire"), "{w:?}");
    // In the witness the flag is credited and the completion's are not; in
    // the all world the reverse.
    assert!(story.runs[1].flags.contains("flag/late"));
    assert!(!story.runs[0].flags.contains("flag/late"));
    // The expiry's `on_expire` fires at the objective's own step.
    let at = "/content/quests/0/objectives/1/limit/on_expire/1";
    assert!(
        story.runs[1].fired.contains(at),
        "{:?}",
        story.runs[1].fired
    );
    assert!(!story.runs[0].fired.contains(at));

    let (code, log, out) = build(&dir);
    assert_eq!(code, 0, "{log}");
    let plan = read(&out, "validation/branch-plan.json");
    assert_eq!(
        endings_of(&plan, 1),
        vec!["ending/late".to_string()],
        "{plan}"
    );
    assert_eq!(
        endings_of(&plan, 0),
        vec!["ending/home".to_string()],
        "{plan}"
    );
    let path = read(&out, "validation/world-path-1.json");
    let steps = path["steps"].as_array().unwrap();
    let e = steps
        .iter()
        .find(|s| s["action"] == "expire")
        .unwrap_or_else(|| panic!("no expire step: {path}"));
    assert_eq!(e["objective"], "obj/exit");
    assert_eq!(e["wait_ticks"], 1200);
    let stand: Vec<i64> = e["stand"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    // Outside the objective's own completion volume, which it keeps out of.
    let ko = e["keep_out"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["objective"] == "obj/exit")
        .expect("the lapse keeps out of its own volume");
    let inside = (0..3).all(|i| {
        ko["lo"][i].as_i64().unwrap() <= stand[i] && stand[i] <= ko["hi"][i].as_i64().unwrap()
    });
    assert!(!inside, "stand {stand:?} is inside {ko}");
    // Where the all world's step for it stood in the order.
    let main = read(&out, "validation/world-path-0.json");
    let pos_main = main["steps"]
        .as_array()
        .unwrap()
        .iter()
        .position(|s| s["objective"] == "obj/exit")
        .unwrap();
    let pos_exp = steps
        .iter()
        .position(|s| s["objective"] == "obj/exit")
        .unwrap();
    assert_eq!(pos_main, pos_exp);
    // The endings ledger: the late ending is reached by the witness alone.
    let endings = read(&out, "validation/endings.json");
    let late = endings["endings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == "ending/late")
        .unwrap();
    assert_eq!(late["worlds"], json!(["world/1"]), "{late}");
}

/// **Perturbation of criterion 5:** the flag written by the completion
/// bundle instead of `on_expire` is credited in the all world, and the
/// witness no longer reaches the late ending.
#[test]
fn the_flag_in_the_completion_bundle_moves_the_credit() {
    let dir = fixture("moved", |v| {
        let q = &mut v["content"]["quests"][0];
        q["objectives"][1]["limit"]["on_expire"] = json!([
            {"type": "narrate", "text": "The keeper bars the door again.",
             "happening": {"text": "the door shuts", "verb": "seals"}}
        ]);
        q["on_objective_complete"]["obj/exit"] = json!([late_flag()]);
    });
    let c = campaign(&dir);
    let flow = Flow::new(&c);
    let story = flow.story();
    let all = story
        .worlds
        .iter()
        .position(|w| w.origin == WorldOrigin::All)
        .unwrap();
    let exp = story
        .worlds
        .iter()
        .position(|w| w.origin == WorldOrigin::Expiry)
        .unwrap();
    assert!(story.runs[all].flags.contains("flag/late"));
    assert!(!story.runs[exp].flags.contains("flag/late"));
}

/// Two limits, two witnesses; a limit in an optional quest is witnessed by a
/// world that performs the quest.
#[test]
fn two_limits_give_two_witnesses_and_an_optional_one_performs_its_quest() {
    let dir = fixture("two", |v| {
        let q = &mut v["content"]["quests"][0];
        q["objectives"][0]["limit"] = json!({"ticks": 2400});
    });
    let c = campaign(&dir);
    let flow = Flow::new(&c);
    let wit: Vec<_> = flow
        .story()
        .worlds
        .iter()
        .filter(|w| w.origin == WorldOrigin::Expiry)
        .collect();
    assert_eq!(wit.len(), 2, "{wit:?}");
    assert!(wit.iter().any(|w| w.performs.contains("obj/talk/expire")));
    assert!(wit.iter().any(|w| w.performs.contains("obj/exit/expire")));
}

/// A limit in an optional quest (`elective-reach`'s `quest/tally`) is
/// witnessed by a world that performs the quest.
#[test]
fn an_optional_quests_limit_is_witnessed_with_its_quest_performed() {
    let dir = scratch("optional");
    common::copy_dir_all(
        &common::compiler_fixtures_dir().join("elective-reach"),
        &dir,
    );
    common::patch_file(&dir.join("quests.json"), |v| {
        // The hello room's door is shut at world load; the opening beat
        // opens it, as `endings.rs`'s copy of this fixture does.
        v["content"]["quests"][0]["on_complete"] = json!([{
            "type": "open-gate", "anchor": "anchor/door",
            "happening": {"subject": "anchor/door", "text": "The Keeper opens the door.",
                          "verb": "opens"}
        }]);
        let o = &mut v["content"]["quests"][1]["objectives"][0];
        o["title"] = json!("Count the bells");
        o["limit"] = json!({"ticks": 2400});
    });
    let c = campaign(&dir);
    let flow = Flow::new(&c);
    let wit: Vec<_> = flow
        .story()
        .worlds
        .iter()
        .filter(|w| w.origin == WorldOrigin::Expiry)
        .collect();
    assert!(!wit.is_empty(), "{:?}", flow.story().worlds);
    for w in &wit {
        assert!(w.performs.contains("obj/count/expire"), "{w:?}");
        assert!(w.performs.contains("quest/tally"), "{w:?}");
    }
    // Off the critical path, the window is counted and not judged (`DW1084`).
    let (code, log, out) = build(&dir);
    assert_eq!(code, 0, "{log}");
    let ledger = read(&out, "validation/limit.json");
    assert_eq!(ledger["limits"][0]["objective"], "obj/count");
    assert!(ledger["limits"][0]["walk_ticks"].is_null(), "{ledger}");
    assert!(log.contains("unwalked, 2400 tick(s) — obj/count"), "{log}");
}
