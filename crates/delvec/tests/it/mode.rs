//! spec-0140 — a mode holds every quest but the ones it keeps. The compiler's
//! half, proven on emitted bytes: the map's mode is one party score every
//! guard of a held quest reads; the entry sets it, stops the held clocks and
//! asks the settlement; a kept quest's completion asks it again; `mode_off`
//! starts the clocks from the seconds they showed; the PackTest owes its
//! templates; and a campaign with no mode emits none of it.
//!
//! The fixture is keep-crawl: `quest/greet-keeper` (talk to the keeper), then
//! `quest/reach-shrine` (arrive, then reach). `mode/audience` keeps the first
//! and is entered from the bundle of its one objective.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};
use serde_json::{Value, json};

const NS: &str = "keep-crawl";
const HELD: &str = " unless score #party dw.mode matches 1";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-mode-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// keep-crawl with `patch` applied to the quests document's `content`.
fn fixture(tag: &str, patch: impl FnOnce(&mut Value)) -> PathBuf {
    let dir = scratch(tag);
    common::copy_dir_all(&common::keep_crawl_dir(), &dir);
    common::patch_file(&dir.join("quests.json"), |v| patch(&mut v["content"]));
    dir
}

/// `mode/audience` keeping the first quest, entered when the keeper is
/// talked to.
fn audience(q: &mut Value) {
    q["modes"] = json!([{"id": "mode/audience", "keeps": ["quest/greet-keeper"]}]);
    // Every objective announced, so each has a line that tells of it.
    q["quests"][0]["objectives"][0]["title"] = json!("Talk to the keeper");
    q["quests"][1]["objectives"][0]["title"] = json!("Enter the crawl");
    q["quests"][1]["objectives"][1]["title"] = json!("Reach the shrine");
    q["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "enter-mode", "mode": "mode/audience"}));
}

/// A minute on the held quest's last objective.
fn minute(q: &mut Value, from: Value) {
    q["quests"][1]["objectives"][1]["limit"] = json!({"ticks": 1200, "from": from});
}

fn with_plan<T>(dir: &Path, f: impl FnOnce(&Plan, &dyn Fn(&Plan) -> BuildOutput) -> T) -> T {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "the fixture must validate: {errors:#?}");
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let build = |plan: &Plan| {
        let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for area in &plan.areas {
            for piece in &area.pieces {
                for t in &piece.templates {
                    let bytes =
                        std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                    structures.insert(t.structure_file.clone(), bytes);
                }
            }
        }
        emit::build(
            plan,
            &loaded.inputs,
            &structures,
            &CommandTree::v1_21_11(),
            &prefabs,
            None,
            &BTreeMap::new(),
        )
        .unwrap_or_else(|e| panic!("the fixture builds: {e:?}"))
    };
    f(&plan, &build)
}

fn build(dir: &Path) -> BuildOutput {
    with_plan(dir, |plan, build| build(plan))
}

fn func(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    String::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("no function `{name}` in the build"))
            .clone(),
    )
    .unwrap()
}

fn template(out: &BuildOutput, name: &str) -> String {
    let path = format!("packtest-datapack/data/{NS}/test/{name}.mcfunction");
    String::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("no template `{name}` in the build"))
            .clone(),
    )
    .unwrap()
}

/// The lines of `text` that dispatch `function`.
fn dispatching<'a>(text: &'a str, function: &str) -> Vec<&'a str> {
    let needle = format!("function {NS}:{function}");
    text.lines()
        .filter(|l| l.ends_with(&needle) || l.contains(&format!("{needle} ")))
        .collect()
}

#[test]
fn a_held_quest_reads_the_mode_in_every_guard_and_a_kept_one_does_not() {
    let out = build(&fixture("guards", audience));
    let tick = func(&out, "tick");
    // The held quest's two objectives: told and completed under the term.
    for f in [
        "announce_o_arrive",
        "complete_o_arrive",
        "announce_o_reach",
        "complete_o_reach",
    ] {
        let lines = dispatching(&tick, f);
        assert!(!lines.is_empty(), "`{f}` is driven from tick");
        for l in lines {
            assert!(l.contains(HELD), "`{f}` is held under the mode: {l}");
            // One term, beside the quest's active score.
            assert!(
                l.contains(&format!(
                    "if score #party dw.qa_reach_shrine matches 1{HELD}"
                )),
                "{l}"
            );
        }
    }
    // The kept quest's objective carries no term.
    let kept = dispatching(&tick, "announce_o_talk");
    assert!(!kept.is_empty());
    for l in kept {
        assert!(!l.contains("dw.mode"), "{l}");
    }
    // `setup` creates the party's entry without moving it.
    let setup = func(&out, "setup");
    assert!(setup.contains("scoreboard objectives add dw.mode dummy"));
    assert!(setup.contains("scoreboard players add #party dw.mode 0"));
    assert!(!setup.contains("scoreboard players set #party dw.mode"));
}

#[test]
fn a_campaign_with_no_mode_emits_none_of_it() {
    let out = build(&fixture("none", |_| {}));
    for (path, bytes) in out.iter() {
        if let Ok(text) = std::str::from_utf8(bytes) {
            assert!(!text.contains("dw.mode"), "{path}");
        }
        assert!(!path.contains("/mode_"), "{path}");
    }
}

#[test]
fn the_entry_sets_the_mode_and_the_settlement_leaves_it() {
    let out = build(&fixture("entry", |q| {
        audience(q);
        q["quests"][1]["on_objective_complete"]["obj/arrive"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "leave-mode", "mode": "mode/audience"}));
    }));
    let enter = func(&out, "mode_audience_enter");
    assert_eq!(
        enter.lines().collect::<Vec<_>>(),
        [
            "execute if score #party dw.mode matches 1 run return 0",
            &format!("function {NS}:mode_off"),
            "scoreboard players set #party dw.mode 1",
            &format!("function {NS}:mode_audience_settle"),
        ]
    );
    let settle = func(&out, "mode_audience_settle");
    assert_eq!(
        settle.lines().collect::<Vec<_>>(),
        [
            "execute unless score #party dw.mode matches 1 run return 0",
            "execute if score #party dw.qa_greet_keeper matches 1 unless score #party dw.q_greet_keeper matches 1 run return 0",
            &format!("function {NS}:mode_off"),
        ]
    );
    assert_eq!(
        func(&out, "mode_off").lines().collect::<Vec<_>>(),
        ["scoreboard players set #party dw.mode 0"]
    );
    // The verbs: the entry in the kept objective's bundle, the leaving a
    // guarded call.
    assert!(func(&out, "complete_o_talk").contains(&format!("function {NS}:mode_audience_enter")));
    assert!(func(&out, "complete_o_arrive").contains(&format!(
        "execute if score #party dw.mode matches 1 run function {NS}:mode_off"
    )));
    // A kept quest's completion asks the settlement, last; a held quest's
    // does not.
    let done = func(&out, "complete_q_greet_keeper");
    assert_eq!(
        done.lines().last(),
        Some(format!("function {NS}:mode_audience_settle").as_str())
    );
    let arm = done
        .lines()
        .position(|l| l.contains("dw.qa_reach_shrine"))
        .expect("the next quest is armed");
    assert!(arm < done.lines().count() - 1);
    assert!(!func(&out, "complete_q_reach_shrine").contains("mode_"));
}

#[test]
fn a_held_clock_stands_still_and_goes_on_from_its_seconds() {
    let out = build(&fixture("clock", |q| {
        audience(q);
        minute(q, json!("activation"));
    }));
    let enter = func(&out, "mode_audience_enter");
    let held: Vec<&str> = enter.lines().skip(3).take(4).collect();
    assert_eq!(
        held,
        [
            "scoreboard players set #mh_reach dw.sys 0",
            &format!(
                "execute store result score #mh_reach dw.sys run schedule clear {NS}:expire_o_reach"
            ),
            &format!(
                "execute if score #mh_reach dw.sys matches 1.. run schedule clear {NS}:limit_tick_o_reach"
            ),
            &format!(
                "execute if score #mh_reach dw.sys matches 1.. run bossbar set {NS}:limit_reach visible false"
            ),
        ]
    );
    assert_eq!(
        func(&out, "mode_off").lines().collect::<Vec<_>>(),
        [
            "scoreboard players set #party dw.mode 0",
            &format!(
                "execute if score #mh_reach dw.sys matches 1.. run function {NS}:mode_resume_o_reach"
            ),
        ]
    );
    assert_eq!(
        func(&out, "mode_resume_o_reach")
            .lines()
            .collect::<Vec<_>>(),
        [
            "scoreboard players set #mh_reach dw.sys 0",
            "execute if score #party dw.os_reach matches 1 run return 0",
            "execute if score #lt_reach dw.sys matches ..0 run scoreboard players set #lt_reach dw.sys 1",
            &format!("bossbar set {NS}:limit_reach visible true"),
            &format!("schedule function {NS}:limit_tick_o_reach 20t replace"),
            "execute store result storage dw:mode t int 20 run scoreboard players get #lt_reach dw.sys",
            &format!("function {NS}:mode_resched_o_reach with storage dw:mode"),
        ]
    );
    assert_eq!(
        func(&out, "mode_resched_o_reach").trim(),
        format!("$schedule function {NS}:expire_o_reach $(t)t replace")
    );
    // The expiry's own guard is the objective's open guard, which is held.
    assert!(func(&out, "expire_o_reach").contains(HELD));
}

#[test]
fn a_held_quest_starts_no_clock() {
    let out = build(&fixture("start", |q| {
        audience(q);
        minute(q, json!("quest-start"));
    }));
    let tick = func(&out, "tick");
    let start = dispatching(&tick, "activate_q_reach_shrine");
    assert_eq!(start.len(), 1);
    assert!(
        start[0].contains(&format!(
            "if score #party dw.qa_reach_shrine matches 1{HELD}"
        )),
        "{}",
        start[0]
    );
}

#[test]
fn a_guard_reads_one_term_per_run_of_the_modes_that_hold_its_quest() {
    let modes = |order: [&str; 3]| {
        let out = build(&fixture(&format!("runs-{}", order[1]), |q| {
            audience(q);
            let keep = |id: &str| {
                let kept = if id == "mode/shrine" {
                    "quest/reach-shrine"
                } else {
                    "quest/greet-keeper"
                };
                json!({"id": id, "keeps": [kept]})
            };
            q["modes"] = json!(order.map(keep));
        }));
        let tick = func(&out, "tick");
        dispatching(&tick, "complete_o_arrive")[0].to_string()
    };
    // Held by the first and the third: two runs.
    let apart = modes(["mode/audience", "mode/shrine", "mode/second-audience"]);
    assert!(
        apart.contains(
            " unless score #party dw.mode matches 1 unless score #party dw.mode matches 3"
        ),
        "{apart}"
    );
    // Held by the first and the second: one run.
    let together = modes(["mode/audience", "mode/second-audience", "mode/shrine"]);
    assert!(
        together.contains(" unless score #party dw.mode matches 1..2"),
        "{together}"
    );
    assert!(!together.contains("matches 3"), "{together}");
}

#[test]
fn the_packtest_owes_a_mode_its_templates() {
    let out = build(&fixture("templates", |q| {
        audience(q);
        minute(q, json!("activation"));
    }));
    let holds = template(&out, "mode_audience_holds");
    // The held objective's guard is read through the engine's own guard, and
    // asserted open, shut under the mode, and open again.
    let reads: Vec<&str> = holds
        .lines()
        .filter(|l| l.starts_with("assert score #pt_md dw.sys matches"))
        .collect();
    assert_eq!(
        reads,
        [
            // before: held completes, held told, kept completes
            "assert score #pt_md dw.sys matches 1",
            "assert score #pt_md dw.sys matches 1",
            "assert score #pt_md dw.sys matches 1",
            // under the mode
            "assert score #pt_md dw.sys matches 0",
            "assert score #pt_md dw.sys matches 0",
            "assert score #pt_md dw.sys matches 1",
            // after
            "assert score #pt_md dw.sys matches 1",
            "assert score #pt_md dw.sys matches 1",
            "assert score #pt_md dw.sys matches 1",
        ],
        "{holds}"
    );
    assert!(holds.contains(HELD), "{holds}");
    // It runs alone, from the whole ledger at the campaign's start.
    assert!(holds.contains(&format!("# @environment {NS}:solo/mode_audience_holds")));
    assert!(holds.contains("scoreboard players set #party dw.campaign 0"));
    assert!(holds.contains(&format!("function {NS}:mode_audience_enter")));
    assert!(holds.contains(&format!("function {NS}:mode_audience_settle")));
    // The clock template runs alone, waits longer than the window with the
    // seconds unmoved, and sees the expiry after the leaving.
    let clock = template(&out, "mode_audience_clock");
    assert!(clock.contains(&format!("# @environment {NS}:solo/mode_audience_clock")));
    assert!(clock.contains("await delay 1225t"), "{clock}");
    // …the expiry never fired while it was held…
    assert!(
        clock
            .lines()
            .any(|l| l == "assert score #lx_reach dw.sys matches 0")
    );
    // …then one second on, one second down and nothing expired…
    assert!(clock.contains("await delay 25t"), "{clock}");
    assert!(
        clock
            .lines()
            .any(|l| l == "assert score #lt_reach dw.sys matches 59")
    );
    assert_eq!(
        clock
            .lines()
            .filter(|l| *l == "assert score #pt_mc dw.sys matches 0")
            .count(),
        2,
        "{clock}"
    );
    // …and the rest of the window.
    assert!(clock.contains("await delay 1180t"), "{clock}");
    assert_eq!(
        clock
            .lines()
            .filter(|l| *l == "assert score #lt_reach dw.sys matches 60")
            .count(),
        2,
        "{clock}"
    );
    assert!(
        clock
            .lines()
            .any(|l| l == "assert score #party dw.x_reach matches 1")
    );
    // No held clock, no clock template.
    let bare = build(&fixture("templates-bare", audience));
    assert!(!bare.contains_key(&format!(
        "packtest-datapack/data/{NS}/test/mode_audience_clock.mcfunction"
    )));
}

/// `DW0807` across a mode: a limit template's window is the server's own
/// clock, so every driver that can enter the mode holding its clock is shut
/// for the whole wait. `obj/reach`'s clock is held by `mode/audience`, entered
/// when `obj/talk` of the KEPT quest completes; a template that left
/// `quest/greet-keeper` in progress — as `mode_audience_holds` does, by design
/// — let a body left on the server complete the talk, enter the mode, hold the
/// clock and never settle the expiry. The kept quest is closed before the
/// clock starts, in each limit template that waits on it; the mode clock
/// template opens it again on purpose, after.
#[test]
fn a_limit_template_shuts_every_other_quest_before_its_clock_runs() {
    let out = build(&fixture("limit-shuts", |q| {
        audience(q);
        minute(q, json!("activation"));
    }));
    let shut = "scoreboard players set #party dw.qa_greet_keeper 0";
    let opened = "scoreboard players set #party dw.qa_greet_keeper 1";
    for name in [
        "limit_o_reach_expires",
        "limit_o_reach_done",
        "party_limit_bar_o_reach",
    ] {
        let t = template(&out, name);
        let lines: Vec<&str> = t.lines().collect();
        let start = lines
            .iter()
            .position(|l| *l == format!("function {NS}:activate_o_reach"))
            .unwrap_or_else(|| panic!("`{name}` starts the clock:\n{t}"));
        let last_shut = lines[..start].iter().rposition(|l| *l == shut);
        assert!(
            last_shut.is_some(),
            "`{name}` shuts the kept quest before the clock starts:\n{t}"
        );
        assert!(
            !lines[last_shut.unwrap()..start].contains(&opened),
            "…and nothing opens it again before the start:\n{t}"
        );
    }
}

#[test]
fn the_binding_counts_what_the_build_read() {
    let dir = fixture("binding", |q| {
        audience(q);
        minute(q, json!("activation"));
    });
    let gate = with_plan(&dir, |plan, _| delvec::compiler::mode::gate(plan));
    assert_eq!(
        gate,
        delvec::compiler::mode::ModeGate {
            modes: 1,
            entered: 1,
            kept: 1,
            held: 1,
            clocks: 1,
            // Entered as the keeper is talked to, left as that quest
            // completes at the same step.
            spans: 1,
            left: 1,
            judged: 0,
        }
    );
    assert!(
        gate.line()
            .starts_with("mode binding (DW1122–DW1124): 1 mode(s), 1 entered")
    );
}

/// The journal ledger hands the bot, per quest, the `#party dw.mode` ranges
/// under which it is held — the same runs its guards read — and the score
/// they are read on, so a held quest's pages are owed when the mode is left
/// and not at its start. A quest no mode holds carries none, and a campaign
/// with no mode names no score.
#[test]
fn the_journal_ledger_names_the_modes_that_hold_each_quest() {
    let ledger = |out: &BuildOutput| -> Value {
        serde_json::from_slice(
            out.get("validation/journal-starts.json")
                .expect("a campaign with a journal writes the ledger"),
        )
        .unwrap()
    };
    let held = |l: &Value, quest: &str| -> Vec<String> {
        let q = l["quests"]
            .as_array()
            .unwrap()
            .iter()
            .find(|q| q["quest"] == quest)
            .unwrap_or_else(|| panic!("no `{quest}` in the ledger: {l}"));
        q["held"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap().to_string())
            .collect()
    };
    let out = build(&fixture("ledger", |q| {
        audience(q);
        q["modes"] = json!([
            {"id": "mode/audience", "keeps": ["quest/greet-keeper"]},
            {"id": "mode/shrine", "keeps": ["quest/reach-shrine"]},
            {"id": "mode/second-audience", "keeps": ["quest/greet-keeper"]},
        ]);
    }));
    let l = ledger(&out);
    assert_eq!(l["mode"], "dw.mode", "{l}");
    assert_eq!(held(&l, "quest/reach-shrine"), ["1", "3"]);
    assert_eq!(held(&l, "quest/greet-keeper"), ["2"]);
    // The ledger's ranges are the guard's: each is one `unless` term.
    let tick = func(&out, "tick");
    let guard = dispatching(&tick, "complete_o_arrive")[0];
    for r in held(&l, "quest/reach-shrine") {
        assert!(
            guard.contains(&format!(" unless score #party dw.mode matches {r}")),
            "{guard}"
        );
    }

    let none = build(&fixture("ledger-none", |q| {
        audience(q);
        q.as_object_mut().unwrap().remove("modes");
        q["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .pop();
    }));
    let l = ledger(&none);
    assert!(l["mode"].is_null(), "{l}");
    assert!(held(&l, "quest/reach-shrine").is_empty(), "{l}");
}

/// The journal's templates own the mode their pages' guards read
/// (`DW0807`'s rule): a quest some mode holds is listed only in no mode, so a
/// sibling template that left a mode on would otherwise decide what the book
/// shows. Each writes `#party dw.mode 0` before its first rewrite, and puts the
/// value it found back at its end.
#[test]
fn a_journal_template_owns_the_mode_its_pages_read() {
    let out = build(&fixture("journal-mode", |q| {
        audience(q);
        q["modes"] = json!([
            {"id": "mode/audience", "keeps": ["quest/greet-keeper"]},
            {"id": "mode/shrine", "keeps": ["quest/reach-shrine"]},
        ]);
    }));
    let own = "scoreboard players set #party dw.mode 0";
    for name in ["guide_journal_pages", "guide_journal_join"] {
        let t = template(&out, name);
        let lines: Vec<&str> = t.lines().collect();
        let first = lines
            .iter()
            .position(|l| {
                l.starts_with(&format!("function {NS}:journal_rewrite"))
                    || l.starts_with(&format!("function {NS}:guide_refresh"))
            })
            .unwrap_or_else(|| panic!("`{name}` writes the book:\n{t}"));
        assert!(
            lines[..first].contains(&own),
            "`{name}` sets the map in no mode before it writes the book:\n{t}"
        );
        assert!(
            lines[first..]
                .iter()
                .any(|l| l.starts_with("scoreboard players operation #party dw.mode = ")),
            "`{name}` puts the mode back:\n{t}"
        );
    }
}

/// keep-crawl with a mode entered mid-story: `quest/a-raid` enters
/// `mode/raid` (keeping it and `kept`) at its one objective, `quest/b-errand`
/// and `quest/c-errand` are a strand `mode/raid` holds, and the finale
/// `quest/reach-shrine` waits on all of them and starts as the raid ends.
/// Every quest after the keeper is started by `greet-keeper`'s completion,
/// except the ones `after_raid` names, which wait on `quest/a-raid` — in the
/// stage-4 plan and the stage-5 trigger both.
fn raid(tag: &str, kept: &[&str], after_raid: &[&str]) -> PathBuf {
    let dir = scratch(tag);
    common::copy_dir_all(&common::keep_crawl_dir(), &dir);
    let side = [
        ("quest/a-raid", "obj/raid", "anchor/keeper-stand"),
        ("quest/b-errand", "obj/errand-b", "anchor/exit"),
        ("quest/c-errand", "obj/errand-c", "anchor/keeper-stand"),
    ];
    let dep = |id: &str| {
        if after_raid.contains(&id) {
            "quest/a-raid"
        } else if id == "quest/c-errand" {
            "quest/b-errand"
        } else {
            "quest/greet-keeper"
        }
    };
    common::patch_file(&dir.join("quest-plan.json"), |v| {
        let quests = v["content"]["quests"].as_array_mut().unwrap();
        for (id, _, _) in side {
            quests.push(json!({
                "act": 1, "activity": ["reach"],
                "area": "area/gatehouse",
                "depends_on": [dep(id)], "goal": format!("Do {id}."), "id": id,
                "mandatory": true, "npcs": []
            }));
        }
        quests[1]["depends_on"] = json!(["quest/a-raid", "quest/b-errand", "quest/c-errand"]);
    });
    common::patch_file(&dir.join("quests.json"), |v| {
        let q = &mut v["content"];
        let mut keeps = vec!["quest/a-raid"];
        keeps.extend_from_slice(kept);
        q["modes"] = json!([{"id": "mode/raid", "keeps": keeps}]);
        q["quests"][1]["trigger"] = json!({"type": "quest-complete", "quest": "quest/a-raid"});
        let quests = q["quests"].as_array_mut().unwrap();
        for (id, obj, anchor) in side {
            let bundle = if id == "quest/a-raid" {
                json!([{"type": "enter-mode", "mode": "mode/raid"}])
            } else {
                json!([])
            };
            quests.push(json!({
                "id": id,
                "happening": {"text": format!("the party takes on {id}"), "verb": "learns"},
                "trigger": {"type": "quest-complete", "quest": dep(id)},
                "objectives": [{
                    "type": "reach-anchor", "id": obj, "anchor": anchor, "radius": 2,
                    "happening": {"text": format!("the party completes {obj}"), "verb": "arrives"}
                }],
                "on_objective_complete": {obj: bundle},
                "on_complete": []
            }));
        }
    });
    dir
}

/// The objectives a built plan's exported path performs, in order.
fn path_of(dir: &Path) -> Result<Vec<String>, delvec::compiler::plan::PlanError> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "the fixture must validate: {errors:#?}");
    Plan::build(&campaign, &prefabs).map(|p| {
        p.critical_path
            .iter()
            .filter_map(|s| s.objective().map(str::to_string))
            .collect()
    })
}

/// The quests story world `k`'s playthrough walks, in order — the
/// whole-world order every story world's path is built on.
fn world_order(dir: &Path) -> Vec<String> {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let flow = delvec::compiler::flow::Flow::new(&campaign);
    flow.playthrough_in(0).quests
}

/// A strand the raid holds, ready beside the quest that enters the raid, is
/// walked before the entry: walked after it, its objectives would be held and
/// the path would stop at the first (`DW1124`). Both the exported path (the
/// finale's closure) and a story world's (every quest) walk it so.
#[test]
fn a_held_strand_ready_beside_the_entry_is_walked_before_the_mode() {
    let dir = raid("before", &["quest/reach-shrine"], &[]);
    let path = path_of(&dir).unwrap_or_else(|e| panic!("the path builds: {}", e.failure.message));
    assert_eq!(
        path,
        [
            "obj/talk",
            "obj/errand-b",
            "obj/errand-c",
            "obj/raid",
            "obj/arrive",
            "obj/reach"
        ]
    );
    assert_eq!(
        world_order(&dir),
        [
            "quest/greet-keeper",
            "quest/b-errand",
            "quest/c-errand",
            "quest/a-raid",
            "quest/reach-shrine"
        ]
    );
}

/// A held quest that can only start once the raid is entered is walked after
/// what the raid keeps, once the settlement has left it.
#[test]
fn a_held_quest_after_the_entry_waits_for_the_mode_to_be_left() {
    // Both errands start on the raid quest's completion; the raid keeps
    // `c-errand` and holds `b-errand`, which is ready first.
    let dir = raid(
        "after",
        &["quest/c-errand"],
        &["quest/b-errand", "quest/c-errand"],
    );
    let path = path_of(&dir).unwrap_or_else(|e| panic!("the path builds: {}", e.failure.message));
    assert_eq!(
        path,
        [
            "obj/talk",
            "obj/raid",
            "obj/errand-c",
            "obj/errand-b",
            "obj/arrive",
            "obj/reach"
        ]
    );
}

/// Where the content forces a held objective inside its mode — the strand
/// starts only once the raid is entered, and the raid keeps the finale, which
/// starts at the same moment — no order exists, and the build still refuses
/// with `DW1124`.
#[test]
fn a_held_objective_the_content_forces_inside_the_mode_is_still_dw1124() {
    let dir = raid("forced", &["quest/reach-shrine"], &["quest/b-errand"]);
    let e = path_of(&dir).expect_err("no order walks the strand outside the raid");
    assert_eq!(
        e.failure.code.to_string(),
        "DW1124",
        "{}",
        e.failure.message
    );
    assert!(
        e.failure.message.contains("`obj/errand-b`") && e.failure.message.contains("`mode/raid`"),
        "{}",
        e.failure.message
    );
}
