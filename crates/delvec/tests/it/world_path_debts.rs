//! A story world's path performs what it reads (`Flow::story_debts`).
//!
//! The fixture is hello-world with a fork the keeper's lore node offers — a
//! dialogue option that completes no objective and sets `flag/trusted` — and an
//! optional quest whose one objective reads `flag/key`. Two acts set the key:
//! an `approach` trigger gated on `flag/trusted`, and a shop offer. The shape is
//! the one a town's stolen-mare world failed on: every world that performs the
//! side quest walked straight to `obj/take`, and the bot waited for a
//! completion no act on its path could cause —
//!
//! * the trusted world took no choice (an option that completes nothing is on
//!   no `talk-to` step) and performed no trigger (its gate never held on the
//!   path), and its `entry_choices` were empty, so the harness's check that the
//!   path takes them passed with nothing to check;
//! * the doubted world, which performs the purchase, carried no mock of it: the
//!   story stance credits a performed act's flag from the start, so the
//!   purchase read as already made and its own `forbids_flags` as shut.
//!
//! The fixed path takes the choice, then the trigger it opens, then the reach;
//! or the purchase by mock, then the reach. A debt no act can pay is `DW0204`.
//!
//! The trigger stands at the latch, off the keeper's walk to the door. A world
//! that is trusted and performs nothing leaves it open and never fires it, so
//! that world's every walk after the choice keeps out of its firing volume —
//! and where the trigger stands at the keeper (the town's Runa, whose fooling
//! opens a trigger she stands inside), no walk can, and the world is `DW0311`.

use crate::common;

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

fn fixture(tag: &str, patch: impl FnOnce(&mut Value, &mut Value)) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("world-path-debts-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    let mut dialogue: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("dialogue.json")).unwrap()).unwrap();
    dialogue["content"]["dialogues"][0]["nodes"][1]["options"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "label": "Tell me a secret.",
            "effects": [{"type": "set-flag", "flag": "flag/trusted"}],
            "happening": {"text": "the keeper trusts the party", "verb": "believes"}
        }));
    let mut quests: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("quests.json")).unwrap()).unwrap();
    {
        let c = &mut quests["content"];
        let cast = c["quests"][0]["cast"].clone();
        c["quests"].as_array_mut().unwrap().push(json!({
            "id": "quest/side", "trigger": {"type": "campaign-start"}, "cast": cast,
            "happening": {"text": "the party takes on quest/side", "verb": "learns"},
            "objectives": [{
                "id": "obj/take", "type": "reach-anchor", "anchor": "anchor/door", "radius": 2,
                "requires_flags": ["flag/key"], "title": "Reach the door with the key",
                "hint": "By the door.",
                "happening": {"text": "the party completes obj/take", "verb": "arrives"}
            }],
            "on_complete": [], "on_objective_complete": {}
        }));
        c["triggers"] = json!([{
            "id": "trigger/key", "at": "anchor/latch", "on": {"on": "approach", "range": 1},
            "requires_flags": ["flag/trusted"], "forbids_flags": ["flag/key"],
            "effects": [{"type": "set-flag", "flag": "flag/key"}]
        }]);
        c["shops"] = json!([{
            "id": "shop/stall", "anchor": "anchor/exit", "title": "The Stall",
            "prop": {"block": "minecraft:barrel[facing=up,open=false]"},
            "offers": [{
                "label": "Buy the key", "forbids_flags": ["flag/key"],
                "effects": [{"type": "set-flag", "flag": "flag/key"}]
            }]
        }]);
    }
    patch(&mut dialogue, &mut quests);
    std::fs::write(dir.join("dialogue.json"), dialogue.to_string()).unwrap();
    std::fs::write(dir.join("quests.json"), quests.to_string()).unwrap();
    common::patch_file(&dir.join("quest-plan.json"), |v| {
        let c = &mut v["content"];
        c["quests"].as_array_mut().unwrap().push(json!({
            "act": 1, "activity": ["reach"], "area": "area/keep", "depends_on": [],
            "goal": "Carry the key to the door.", "id": "quest/side", "mandatory": false,
            "npcs": []
        }));
        c["branch_points"] = json!([{
            "id": "branch-point/trust", "forks_on": ["flag/trusted", "flag/key"],
            "opens_at": "quest/open-the-door",
            "branches": [
                {"id": "branch/trusted", "flags": ["flag/trusted", "flag/key"],
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

/// Each step of a world's path as `(action, what it names)`.
fn steps(out: &Path, row: &Value) -> Vec<(String, String)> {
    let path = read(out, row["path"].as_str().expect("the world exports a path"));
    path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            let name = ["objective", "trigger", "act", "command"]
                .iter()
                .find_map(|k| s[*k].as_str())
                .unwrap_or("")
                .to_string();
            (s["action"].as_str().unwrap().to_string(), name)
        })
        .collect()
}

/// The world rows that perform the side quest: every one of them reads
/// `flag/key` at `obj/take`.
fn side_worlds(plan: &Value) -> Vec<&Value> {
    plan["branches"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| {
            b["participation"]["performs"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a == "quest/side")
        })
        .collect()
}

fn position(steps: &[(String, String)], action: &str, name: &str) -> usize {
    steps
        .iter()
        .position(|(a, n)| a == action && n == name)
        .unwrap_or_else(|| panic!("no `{action}` step for `{name}` on {steps:?}"))
}

#[test]
fn every_world_that_reads_the_key_performs_an_act_that_sets_it() {
    let (code, log, out) = build(&fixture("paid", |_, _| {}));
    assert_eq!(code, 0, "{log}");
    let plan = read(&out, "branch-plan.json");
    let worlds = side_worlds(&plan);
    // Binding: the trusted all world and the doubted all world both perform
    // the side quest; a fixture where neither did would prove nothing.
    assert_eq!(worlds.len(), 2, "{plan:#}");
    let choice = "/trigger dw.dlg_keeper set 4";
    for row in worlds {
        let s = steps(&out, row);
        let take = position(&s, "reach", "obj/take");
        let trusted = row["selection"]["alternatives"][0] == json!(["flag/trusted"]);
        if trusted {
            // The selection's choice, then the trigger it opens, then the read.
            let chose = position(&s, "choose", choice);
            let key = position(&s, "trigger", "trigger/key");
            assert!(chose < key && key < take, "{} walks {s:?}", row["id"]);
            assert_eq!(
                row["entry_choices"],
                json!([{"npc": "npc/keeper", "option": 4, "command": choice}]),
                "{} names the choice that enters it",
                row["id"]
            );
        } else {
            // Nothing opens the trigger here: the purchase pays, by mock.
            let bought = position(&s, "mock", "shop/stall/offer-0");
            assert!(bought < take, "{} walks {s:?}", row["id"]);
            assert!(
                !s.iter().any(|(a, _)| a == "choose"),
                "{} declines the choice and takes none: {s:?}",
                row["id"]
            );
        }
    }
    // The choice step is walked to the keeper and proven by the flag it sets.
    let path = read(&out, "world-path-0.json");
    let chose = path["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["action"] == "choose")
        .expect("world/0 takes the choice");
    assert_eq!(chose["npc"], "npc/keeper");
    assert_eq!(
        chose["sets"],
        json!([{"objective": "dw.f_trusted", "holder": "#party", "min": 1, "max": 1,
                "negate": false}])
    );
    assert!(chose.get("objective").is_none(), "{chose}");
}

#[test]
fn a_selection_no_point_of_the_path_offers_is_dw0204() {
    // The trusted option asks for a flag only the delve's last step sets.
    let (code, log, _) = build(&fixture("unoffered", |d, q| {
        let opts = d["content"]["dialogues"][0]["nodes"][1]["options"]
            .as_array_mut()
            .unwrap();
        opts.last_mut().unwrap()["requires_flags"] = json!(["flag/late"]);
        q["content"]["quests"][0]["on_objective_complete"]["obj/exit"] =
            json!([{"type": "set-flag", "flag": "flag/late"}]);
    }));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.lines().any(|l| l.starts_with("DW0204 [error]")
            && l.contains("its selection takes option #4 of `npc/keeper`")),
        "{log}"
    );
}

#[test]
fn a_read_no_act_of_the_world_can_pay_is_dw0204() {
    // The purchase sets the key only on four coins, and the keeper pays one:
    // the doubted world, which has no other way to the key, cannot read it.
    let (code, log, _) = build(&fixture("unpaid", |_, q| {
        let c = &mut q["content"];
        c["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "add-state", "state": "state/coins", "amount": 1}));
        c["state"] = json!([{"id": "state/coins", "initial": 0, "name": "Coins",
                             "note": "What the stall asks.", "scope": "party"}]);
        c["shops"][0]["offers"][0]["effects"][0]["when"] =
            json!({"requires_state": [{"op": "at-least", "state": "state/coins", "value": 4}]});
    }));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.lines().any(|l| l.starts_with("DW0204 [error]")
            && l.contains("reads `flag/key`, and no act this world performs sets it there")),
        "{log}"
    );
}

/// Every keep-out row of a world's path step, as `(act key, act, lo, hi)`.
fn keep_out_rows(step: &Value) -> Vec<(String, String, [i64; 3], [i64; 3])> {
    let v3 = |v: &Value| -> [i64; 3] {
        let a = v.as_array().unwrap();
        [
            a[0].as_i64().unwrap(),
            a[1].as_i64().unwrap(),
            a[2].as_i64().unwrap(),
        ]
    };
    step["keep_out"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|k| {
            let (key, act) = ["objective", "trigger"]
                .iter()
                .find_map(|key| Some((*key, k[*key].as_str()?)))
                .expect("a keep-out row names its act");
            (key.to_string(), act.to_string(), v3(&k["lo"]), v3(&k["hi"]))
        })
        .collect()
}

#[test]
fn a_world_that_leaves_a_trigger_open_and_never_fires_it_walks_round_it() {
    let (code, log, out) = build(&fixture("open", |_, _| {}));
    assert_eq!(code, 0, "{log}");
    let plan = read(&out, "branch-plan.json");
    // world/1: trusted, performs nothing — the trigger's gate is open after
    // the choice and the world never fires it.
    let row = plan["branches"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| {
            b["selection"]["alternatives"][0] == json!(["flag/trusted"])
                && b["participation"]["performs"]
                    .as_array()
                    .unwrap()
                    .is_empty()
        })
        .expect("the trusted world that performs nothing");
    let path = read(&out, row["path"].as_str().unwrap());
    let steps = path["steps"].as_array().unwrap();
    let chose = steps
        .iter()
        .position(|s| s["action"] == "choose")
        .expect("the world takes the choice");
    let mut boxes = Vec::new();
    for (i, s) in steps.iter().enumerate() {
        let rows: Vec<_> = keep_out_rows(s)
            .into_iter()
            .filter(|(key, act, _, _)| key == "trigger" && act == "trigger/key")
            .collect();
        if i <= chose || s["pos"].is_null() {
            // Before the choice the gate is shut; a step that stands nowhere
            // walks nowhere.
            assert!(rows.is_empty(), "step {i} keeps out of a shut trigger: {s}");
        } else {
            assert!(
                !rows.is_empty(),
                "step {i} walks beside the open trigger: {s}"
            );
            boxes.extend(rows);
        }
    }
    // The binding: the latch's firing volume, kept out of by the walks after
    // the choice.
    assert!(!boxes.is_empty(), "{path:#}");
    let way = read(
        &out,
        &row["path"]
            .as_str()
            .unwrap()
            .replace("world-path-", "world-waypoints-"),
    );
    let inside = |c: &Value| {
        boxes.iter().any(|(_, _, lo, hi)| {
            (0..3).all(|a| lo[a] <= c[a].as_i64().unwrap() && c[a].as_i64().unwrap() <= hi[a])
        })
    };
    let walked: Vec<&Value> = way["legs"]
        .as_array()
        .unwrap()
        .iter()
        .skip(1)
        .flat_map(|l| l["waypoints"].as_array().unwrap())
        .collect();
    assert!(!walked.is_empty(), "{way:#}");
    assert!(!walked.iter().any(|c| inside(c)), "{way:#}");
    // The world that performs the trigger keeps out of none of it.
    for b in plan["branches"].as_array().unwrap() {
        let performs = b["participation"]["performs"].as_array().unwrap();
        if !performs.iter().any(|a| a == "trigger/key") {
            continue;
        }
        let p = read(&out, b["path"].as_str().unwrap());
        for s in p["steps"].as_array().unwrap() {
            assert!(
                keep_out_rows(s)
                    .iter()
                    .all(|(key, _, _, _)| key != "trigger"),
                "{} performs the trigger and keeps out of it: {s}",
                b["id"]
            );
        }
    }
}

#[test]
fn a_world_whose_choice_opens_a_trigger_where_the_party_stands_is_dw0311() {
    // The trigger stands at the keeper, inside reach of the choice that opens
    // it: the trusted world that never fires it cannot be walked.
    let (code, log, _) = build(&fixture("stood-in", |_, q| {
        q["content"]["triggers"][0]["at"] = json!("anchor/keeper-stand");
        q["content"]["triggers"][0]["on"] = json!({"on": "approach", "range": 3});
    }));
    assert_ne!(code, 0, "{log}");
    assert!(
        log.lines().any(|l| l.starts_with("DW0311 [error]")
            && l.contains("world/1")
            && l.contains("the firing volume of `trigger/key`")
            && l.contains("never performs")),
        "{log}"
    );
}

#[test]
fn a_trigger_that_only_presents_is_walked_past_where_one_that_writes_is_refused() {
    // A second trigger at the keeper, opened by the same choice and never
    // performed — but its bundle only narrates, so firing it leaves the world
    // the one planned: no walk keeps out of it, and the build stands.
    let (code, log, out) = build(&fixture("presents", |_, q| {
        q["content"]["triggers"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "trigger/murmur", "at": "anchor/keeper-stand",
                "on": {"on": "approach", "range": 3},
                "requires_flags": ["flag/trusted"],
                "effects": [{"type": "narrate", "style": "chat", "text": "The keeper murmurs."}]
            }));
    }));
    assert_eq!(code, 0, "{log}");
    let plan = read(&out, "branch-plan.json");
    let mut walked = 0;
    for b in plan["branches"].as_array().unwrap() {
        let Some(p) = b["path"].as_str() else {
            continue;
        };
        for s in read(&out, p)["steps"].as_array().unwrap() {
            walked += 1;
            assert!(
                keep_out_rows(s)
                    .iter()
                    .all(|(_, act, _, _)| act != "trigger/murmur"),
                "{} keeps out of a trigger that only presents: {s}",
                b["id"]
            );
        }
    }
    assert!(walked > 0, "{plan:#}");
}
