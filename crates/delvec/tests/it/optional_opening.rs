//! **A way an optional quest opens is open for that quest's own later legs**,
//! on a path that plays it, and on no other path.
//!
//! The fixture is hello-world with its one door taken from the finale: the
//! keeper's talk completes the delve and opens nothing. An optional quest
//! (spec-0051) opens the door when its first objective is reached at the
//! keeper's stand, and its second objective stands behind the door. A
//! dialogue fork (a branch point on `flag/trusted`) makes the build walk
//! story worlds (spec-0108), and the all world of each selection performs the
//! optional quest, so its two legs are a story world's path.
//!
//! The shape is the festival town's cat errand: the loom over the hut's
//! trapdoor is moved by the errand's own `open-gate`, and the next leg goes
//! down through it. A path that plays the objective is a world whose party
//! played it; reading its `open-gate` as unforced there shut the way in front
//! of the very party that opened it, and the world was refused `DW0311`.

use crate::common;

use std::path::{Path, PathBuf};

use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::{Value, json};

/// The fixture, with `finale_exit` deciding whether the MANDATORY finale also
/// owes a walk through the door (to `anchor/exit`, after the talk) that only
/// the optional quest opens.
fn fixture(tag: &str, finale_exit: bool) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("optional-opening-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    common::patch_file(&dir.join("dialogue.json"), |d| {
        d["content"]["dialogues"][0]["nodes"][1]["options"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "label": "Tell me a secret.",
                "effects": [{"type": "set-flag", "flag": "flag/trusted"}],
                "happening": {"text": "the keeper trusts the party", "verb": "believes"}
            }));
    });
    common::patch_file(&dir.join("quests.json"), |q| {
        let c = &mut q["content"];
        let finale = &mut c["quests"][0];
        // The finale opens nothing.
        finale["on_objective_complete"] = json!({});
        if !finale_exit {
            let objectives = finale["objectives"].as_array_mut().unwrap();
            objectives.retain(|o| o["id"] != "obj/exit");
        }
        let cast = finale["cast"].clone();
        c["quests"].as_array_mut().unwrap().push(json!({
            "id": "quest/side", "trigger": {"type": "campaign-start"}, "cast": cast,
            "happening": {"text": "the party takes on quest/side", "verb": "learns"},
            "objectives": [
                {
                    "id": "obj/look", "type": "reach-anchor", "anchor": "anchor/keeper-stand",
                    "radius": 2, "title": "Look in on the keeper's stand",
                    "hint": "Where the keeper stands.",
                    "happening": {"text": "the party completes obj/look", "verb": "arrives"}
                },
                {
                    "id": "obj/through", "type": "reach-anchor", "anchor": "anchor/exit",
                    "radius": 2, "after": ["obj/look"], "title": "Go through the door",
                    "hint": "Through the door the stand opens.",
                    "happening": {"text": "the party completes obj/through", "verb": "arrives"}
                }
            ],
            "on_objective_complete": {
                "obj/look": [{
                    "type": "open-gate", "anchor": "anchor/door",
                    "happening": {"text": "the gate at anchor/door opens", "verb": "opens"}
                }]
            },
            "on_complete": []
        }));
    });
    common::patch_file(&dir.join("quest-plan.json"), |v| {
        let c = &mut v["content"];
        c["quests"].as_array_mut().unwrap().push(json!({
            "act": 1, "activity": ["reach"], "area": "area/keep", "depends_on": [],
            "goal": "Look in on the stand and go through the door.", "id": "quest/side",
            "mandatory": false, "npcs": []
        }));
        c["branch_points"] = json!([{
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
        &std::fs::read_to_string(out.join("validation").join(rel))
            .unwrap_or_else(|e| panic!("{rel}: {e}")),
    )
    .unwrap()
}

/// A story world that performs the optional quest walks through the door the
/// quest's first objective opens, to its second objective behind it.
#[test]
fn an_optional_quests_opening_is_open_for_its_own_later_legs() {
    let (code, log, out) = build(&fixture("own-legs", false));
    assert_eq!(
        code, 0,
        "a world that plays `obj/look` has opened the door it walks through next: {log}"
    );
    // Binding: the worlds that perform the side quest, each with a path that
    // reaches `obj/through` after `obj/look`. A fixture where no world walked
    // the quest would prove nothing about its legs.
    let plan = read(&out, "branch-plan.json");
    let mut walked = 0usize;
    for row in plan["branches"].as_array().unwrap() {
        let performs = row["participation"]["performs"].as_array().unwrap();
        if !performs.iter().any(|a| a == "quest/side") {
            continue;
        }
        let path = read(
            &out,
            row["path"].as_str().expect("the world exports a path"),
        );
        let objectives: Vec<&str> = path["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["objective"].as_str())
            .collect();
        let look = objectives.iter().position(|o| *o == "obj/look");
        let through = objectives.iter().position(|o| *o == "obj/through");
        assert!(
            matches!((look, through), (Some(l), Some(t)) if l < t),
            "{} walks {objectives:?}",
            row["id"]
        );
        walked += 1;
    }
    assert!(walked >= 2, "the all world of each selection: {plan:#}");
}

/// The complement: a path that does not play the optional quest does not see
/// what it opens. The exported path plays the finale's closure alone, so the
/// optional `open-gate` has no step there and the region model holds no
/// opening for it; the finale's own walk through the same door is refused.
#[test]
fn an_optional_quests_opening_is_shut_on_a_path_that_does_not_play_it() {
    let dir = fixture("mandatory-exit", true);
    let loaded = load_campaign_dir(&dir).expect("fixture loads");
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    // Binding: the exported path walks the finale to `obj/exit` and never
    // plays the optional quest.
    assert!(
        plan.objective_steps.contains_key("obj/exit"),
        "{:?}",
        plan.objective_steps
    );
    assert!(
        !plan.objective_steps.contains_key("obj/look"),
        "{:?}",
        plan.objective_steps
    );
    assert!(
        plan.region_events.iter().all(|e| e.fills()),
        "the optional `open-gate` is unforced on a path that does not play it, and an \
         unforced opening is never credited: {:#?}",
        plan.region_events
    );
    let (code, log, _) = build(&dir);
    assert_eq!(code, 3, "{log}");
    assert!(
        (log.contains("DW0306") || log.contains("DW0311"))
            && log.contains("obj/exit")
            && log.contains("anchor/door"),
        "the finale's walk to `obj/exit` crosses a door only the optional quest opens: {log}"
    );
}
