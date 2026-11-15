//! A task the story loses (spec-0133 §1 as amended): `fail-objective` settles
//! an objective **failed** — whatever its own gate says, its completion bundle
//! and its `on_expire` unfired, its quest going on — and a clock settles its
//! objective whatever the objective's gate says (§3 as amended).
//!
//! The fixture is hello-world with a moth seated when the keeper is spoken
//! to; a credited kill of it fails the door (`obj/exit`), which carries a
//! minute's `limit` and a gate the talk opens. Two endings read the flag the
//! kill writes, the fork declared.

use crate::common;

use std::path::{Path, PathBuf};

use delvec::compiler::flow::{Flow, WorldOrigin};
use delvewright_dsl::parse_campaign;
use serde_json::{Value, json};

const NS: &str = "hello-world";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-fail-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn happening(text: &str, verb: &str) -> Value {
    json!({"text": text, "verb": verb})
}

/// The moth's `on_kill`: the flag, the journal's line, and the loss.
fn moth_kill() -> Value {
    json!([
        {"type": "set-flag", "flag": "flag/moth-lost",
         "happening": happening("the moth is lost", "learns")},
        {"type": "record-outcome", "objective": "obj/exit",
         "line": "We swatted the keeper's moth, and he kept the door shut after that."},
        {"type": "fail-objective", "objective": "obj/exit"}
    ])
}

fn fixture(tag: &str, patch: impl FnOnce(&mut Value)) -> PathBuf {
    let dir = scratch(tag);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    common::patch_file(&dir.join("quest-plan.json"), |v| {
        v["content"]["branch_points"] = json!([{
            "id": "branch-point/moth", "forks_on": ["flag/moth-lost"],
            "opens_at": "quest/open-the-door",
            "branches": [
                {"id": "branch/lost", "flags": ["flag/moth-lost"], "leads_to": "quest/open-the-door"},
                {"id": "branch/kept", "flags": [], "leads_to": "quest/open-the-door"}
            ]
        }]);
    });
    common::patch_file(&dir.join("quests.json"), |v| {
        v["content"]["waves"] = json!([{
            "id": "wave/moth", "anchor": "anchor/keeper-stand",
            "mobs": [{"entity": "minecraft:bat", "count": 1,
                      "attributes": {"follow_range": 1.0}}],
            "on_kill": {"effects": moth_kill()}
        }]);
        let q = &mut v["content"]["quests"][0];
        let talk = q["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap();
        talk.push(json!({"type": "spawn-wave", "wave": "wave/moth",
            "happening": {"subject": "wave/moth", "text": "a moth flutters by the keeper",
                          "verb": "arrives"}}));
        talk.push(json!({"type": "set-flag", "flag": "flag/told",
            "happening": happening("the keeper tells of the door", "learns")}));
        q["objectives"][1]["requires_flags"] = json!(["flag/told"]);
        q["objectives"][1]["record"] = json!("The keeper's door would stay open only a minute.");
        q["objectives"][1]["limit"] = json!({
            "ticks": 1200,
            "on_expire": [{"type": "narrate", "text": "The keeper bars the door again.",
                           "happening": happening("the door shuts", "seals")}]
        });
        q["on_complete"] = json!([
            {"type": "campaign-complete", "ending": "ending/lost",
             "when": {"requires_flags": ["flag/moth-lost"]},
             "happening": happening("lost.", "survives")},
            {"type": "campaign-complete", "ending": "ending/home",
             "when": {"forbids_flags": ["flag/moth-lost"]},
             "happening": happening("home.", "survives")}
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

fn built(dir: &Path) -> PathBuf {
    let (code, log, out) = build(dir);
    assert_eq!(code, 0, "{log}");
    out
}

fn func(out: &Path, name: &str) -> String {
    std::fs::read_to_string(out.join(format!("datapack/data/{NS}/function/{name}.mcfunction")))
        .unwrap_or_else(|e| panic!("no function `{name}`: {e}"))
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

/// The codes `delvec validate` refuses the fixture with.
fn refusals(dir: &Path) -> String {
    let (code, log, _) = build(dir);
    assert_ne!(code, 0, "the fixture must be refused:\n{log}");
    log
}

#[test]
fn the_schema_carries_fail_objective() {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(["schema", "--stage", "quests"])
        .output()
        .expect("delvec runs");
    let schema = String::from_utf8_lossy(&o.stdout);
    assert!(schema.contains("\"fail-objective\""), "{schema}");
    let e: delvewright_dsl::QuestEffect =
        serde_json::from_value(json!({"type": "fail-objective", "objective": "obj/exit"})).unwrap();
    assert_eq!(e.fail_objective().map(|o| o.as_str()), Some("obj/exit"));
    assert_eq!(e.verb.tag(), "fail-objective");
}

/// The loss settles the objective failed and settled, stops its clock, tells
/// a party that was told of it, and asks its quest; nothing of its completion
/// bundle or its `on_expire` runs. The bundle that fails it guards on the
/// settled score, and every guard of the objective reads the failed score.
#[test]
fn a_failed_objective_settles_and_its_quest_goes_on() {
    let out = built(&fixture("settle", |_| {}));
    let settle = func(&out, "settle_failed_o_exit");
    for line in [
        "scoreboard players set #party dw.of_exit 1",
        "scoreboard players set #party dw.os_exit 1",
        &format!("schedule clear {NS}:expire_o_exit"),
        &format!("bossbar remove {NS}:limit_exit"),
        &format!("function {NS}:check_q_open_the_door"),
    ] {
        assert!(settle.lines().any(|l| l == line), "`{line}` in\n{settle}");
    }
    assert!(
        settle.contains("execute if score #party dw.ann_exit matches 1 run tellraw @a")
            && settle.contains("delvewright.ui.limit.failed"),
        "the closing is told only to a party told of the task\n{settle}"
    );
    assert!(
        !settle.contains("The keeper bars the door again")
            && !settle.contains("dw:complete")
            && !settle.contains("dw:expired"),
        "nothing of the completion or the expiry runs\n{settle}"
    );
    let kill = func(&out, "on_kill_w_moth");
    assert!(
        kill.lines().any(|l| l
            == format!(
                "execute unless score #party dw.os_exit matches 1 run function {NS}:settle_failed_o_exit"
            )),
        "{kill}"
    );
    // The quest counts the failed objective as settled.
    let check = func(&out, "check_q_open_the_door");
    assert!(
        check.contains("if score #party dw.os_exit matches 1"),
        "{check}"
    );
    // Every driver of the objective is closed by its failure.
    let tick = func(&out, "tick");
    let drivers: Vec<&str> = tick
        .lines()
        .filter(|l| l.contains("complete_o_exit") || l.contains("announce_o_exit"))
        .collect();
    assert!(!drivers.is_empty(), "{tick}");
    for d in drivers {
        assert!(
            d.contains("unless score #party dw.of_exit matches 1"),
            "{d}"
        );
    }
    // The journal's finished pages are written under the settled score.
    let journal = func(&out, "journal_write");
    assert!(
        journal.contains("if score #party dw.os_exit matches 1"),
        "{journal}"
    );
}

/// **The expiry is never held by the gate** (spec-0133 §3 as amended): the
/// clock settles its objective when it runs out whatever the objective's gate
/// says, and so does the born-expired driver. Before the amendment both read
/// the objective's whole pending guard, gate included, so a task whose gate a
/// lost body held shut could neither complete nor expire.
#[test]
fn an_expiry_settles_whatever_the_gate_says() {
    let out = built(&fixture("gate", |_| {}));
    let exp = func(&out, "expire_o_exit");
    assert!(
        exp.contains(&format!("run function {NS}:settle_expired_o_exit")),
        "{exp}"
    );
    assert!(
        !exp.contains("dw.f_told"),
        "the expiry reads the objective's gate:\n{exp}"
    );
    for term in [
        "if score #party dw.qa_open_the_door matches 1",
        "if score #party dw.o_talk matches 1",
        "unless score #party dw.o_exit matches 1",
        "unless score #party dw.x_exit matches 1",
        "unless score #party dw.of_exit matches 1",
    ] {
        assert!(exp.contains(term), "`{term}` in\n{exp}");
    }
    // The completion still reads the gate: only the clock ignores it.
    let tick = func(&out, "tick");
    let complete = tick
        .lines()
        .find(|l| l.contains("run function hello-world:complete_o_exit"))
        .unwrap_or_else(|| panic!("{tick}"));
    assert!(
        complete.contains("if score #party dw.f_told matches 1"),
        "{complete}"
    );
    // A clock that starts before its objective opens: the born-expired driver
    // reads the open guard, never the gate.
    let out = built(&fixture("born", |v| {
        v["content"]["quests"][0]["objectives"][1]["limit"]["from"] = json!("quest-start");
    }));
    let tick = func(&out, "tick");
    let born = tick
        .lines()
        .find(|l| l.starts_with("execute if score #lx_exit dw.sys matches 1"))
        .unwrap_or_else(|| panic!("no born-expired driver in\n{tick}"));
    assert!(!born.contains("dw.f_told"), "{born}");
    assert!(
        born.ends_with(&format!("run function {NS}:settle_expired_o_exit")),
        "{born}"
    );
}

/// **A kill that loses a task is a world**: the all world does not perform
/// it, one failure witness does, its deed performed in front of the step of
/// the objective it fails; that step is no act of the party's, and the mock
/// that performs the kill asserts the failure.
#[test]
fn a_kill_that_loses_a_task_is_a_world() {
    let dir = fixture("world", |_| {});
    let c = campaign(&dir);
    let flow = Flow::new(&c);
    let story = flow.story();
    let origins: Vec<WorldOrigin> = story.worlds.iter().map(|w| w.origin).collect();
    assert_eq!(
        origins,
        vec![WorldOrigin::All, WorldOrigin::Expiry, WorldOrigin::Failure],
        "{:?}",
        story.worlds
    );
    assert!(!story.worlds[0].performs.contains("wave/moth"));
    assert!(story.worlds[2].performs.contains("wave/moth"));
    assert!(
        story.runs[2].incoherent.is_none(),
        "{:?}",
        story.runs[2].incoherent
    );
    assert!(story.runs[2].failed.contains("obj/exit"));
    assert!(story.runs[0].failed.is_empty());
    assert!(story.runs[2].flags.contains("flag/moth-lost"));

    let out = built(&dir);
    let path = read(&out, "validation/world-path-2.json");
    let steps = path["steps"].as_array().unwrap();
    assert!(
        !steps.iter().any(|s| s["objective"] == "obj/exit"),
        "the failed objective has no step: {path}"
    );
    let mock = steps
        .iter()
        .find(|s| s["action"] == "mock")
        .unwrap_or_else(|| panic!("no mock: {path}"));
    assert_eq!(mock["act"], "wave/moth");
    assert_eq!(mock["function"], format!("{NS}:on_kill_w_moth"));
    let term = |list: &str| {
        mock[list]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["objective"] == "dw.of_exit")
            .cloned()
            .unwrap_or_else(|| panic!("no failed term in `{list}`: {mock}"))
    };
    assert_eq!(term("after")["negate"], json!(false), "{mock}");
    assert_eq!(term("before")["negate"], json!(true), "{mock}");
    // The ending the loss leads to is reached by the failure world alone.
    let endings = read(&out, "validation/endings.json");
    let lost = endings["endings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["id"] == "ending/lost")
        .unwrap();
    assert_eq!(lost["worlds"], json!(["world/2"]), "{lost}");
    // The journal ledger: the door settles failed there, with the kill's line.
    let records = read(&out, "validation/journal-records.json");
    let row = records["worlds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["world"] == "world/2")
        .and_then(|w| w["finished"].as_array().unwrap().first().cloned())
        .unwrap_or_else(|| panic!("{records}"));
    assert_eq!(row["settles"], "failed", "{records}");
    assert_eq!(row["written"], json!([1]), "{records}");
}

/// **Perturbation**: the same kill without the loss is an ordinary ambient
/// act — no failure world, the all world performs it.
#[test]
fn without_the_loss_the_kill_is_no_world_of_its_own() {
    let dir = fixture("no-loss", |v| {
        v["content"]["waves"][0]["on_kill"]["effects"]
            .as_array_mut()
            .unwrap()
            .pop();
    });
    let c = campaign(&dir);
    let flow = Flow::new(&c);
    let story = flow.story();
    assert!(
        !story
            .worlds
            .iter()
            .any(|w| w.origin == WorldOrigin::Failure),
        "{:?}",
        story.worlds
    );
    assert!(story.worlds[0].performs.contains("wave/moth"));
}

/// `DW1111`: a loss in a bundle that fires only once its objective has
/// settled can fail nothing — its own completion, its own `on_expire`, its
/// quest's `on_complete`.
#[test]
fn dw1111_a_failure_that_never_takes() {
    let lose = json!({"type": "fail-objective", "objective": "obj/exit"});
    for (tag, at) in [("own", 0), ("expire", 1), ("quest", 2)] {
        let lose = lose.clone();
        let dir = fixture(&format!("dw1111-{tag}"), move |v| {
            let q = &mut v["content"]["quests"][0];
            match at {
                0 => q["on_objective_complete"]["obj/exit"] = json!([lose]),
                1 => q["objectives"][1]["limit"]["on_expire"]
                    .as_array_mut()
                    .unwrap()
                    .push(lose),
                _ => q["on_complete"].as_array_mut().unwrap().push(lose),
            }
        });
        let log = refusals(&dir);
        assert!(log.contains("DW1111"), "{tag}: {log}");
    }
    // Passing: the same loss from another objective's completion.
    let dir = fixture("dw1111-ok", |v| {
        v["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "fail-objective", "objective": "obj/exit",
                         "when": {"requires_flags": ["flag/moth-lost"]}}));
    });
    let (code, log, _) = build(&dir);
    assert!(!log.contains("DW1111"), "{log}");
    assert_eq!(code, 0, "{log}");
}

/// `DW0112`: a loss naming an objective no quest declares.
#[test]
fn a_failure_of_an_undeclared_objective_is_a_dangling_reference() {
    let dir = fixture("dangling", |v| {
        v["content"]["waves"][0]["on_kill"]["effects"][2]["objective"] = json!("obj/nowhere");
    });
    let log = refusals(&dir);
    assert!(
        log.contains("DW0112") && log.contains("obj/nowhere"),
        "{log}"
    );
}

/// `DW1112`: a loss in a root no story world dates — the campaign's
/// `on_death`.
#[test]
fn dw1112_a_failure_no_world_dates() {
    let dir = fixture("dw1112", |v| {
        v["content"]["on_death"] = json!([{"type": "fail-objective", "objective": "obj/exit"}]);
    });
    let log = refusals(&dir);
    assert!(log.contains("DW1112"), "{log}");
    // The binding line states what it read.
    let ok = fixture("dw1112-ok", |_| {});
    let (code, log, _) = build(&ok);
    assert_eq!(code, 0, "{log}");
    assert!(
        log.contains(
            "failure binding (DW1112): 1 fail-objective site(s) — 0 in a quest bundle, 1 in 1 \
             elective act(s), each a failure world — 0 refused"
        ),
        "{log}"
    );
}

/// **A drive holds an objective pending by every settlement its guard
/// reads.** A press that serves a limited objective which a loss can also
/// fail is dispatched under the objective's serving guard, which reads the
/// expired score and the failed score both; the press's PackTest drives the
/// objective pending, so it clears both — a sibling that left the objective
/// failed would otherwise shut the press it means to prove. The drive
/// cleared the expired score alone.
#[test]
fn a_served_limited_objective_that_can_fail_is_driven_clear_of_both() {
    let out = built(&fixture("served", |v| {
        let c = &mut v["content"];
        c["state"] = json!([{ "id": "state/turns", "initial": 0, "name": "Turns",
                              "scope": "party", "note": "How far the crank is wound." }]);
        c["triggers"] = json!([{
            "id": "trigger/crank", "at": "anchor/latch", "on": { "on": "use" },
            "once": false,
            "prop": { "block": "minecraft:stone_button[face=floor,facing=north]" },
            "effects": [{ "type": "add-state", "state": "state/turns", "amount": 1 }],
            "serves": { "objective": "obj/exit" }
        }]);
    }));
    let test = std::fs::read_to_string(out.join(format!(
        "packtest-datapack/data/{NS}/test/env_trigger_crank.mcfunction"
    )))
    .expect("the press's template");
    let guard = func(&out, "trig_crank");
    for score in ["dw.x_exit", "dw.of_exit"] {
        assert!(
            guard.contains(&format!("unless score #party {score} matches 1")),
            "the serving guard reads `{score}`:\n{guard}"
        );
        assert!(
            test.lines()
                .any(|l| l == format!("scoreboard players set #party {score} 0")),
            "the drive clears `{score}`:\n{test}"
        );
    }
}
