//! spec-0133 — a beat settles two ways. The compiler's half of the timed
//! beat, proven on emitted bytes: the clock is scheduled at the declared tick
//! on the start event the limit names (§3), cleared at completion, and the
//! expiry settles only a pending objective, marks it, tells the party and
//! fires `on_expire`; every guard of the objective reads the expired score;
//! the countdown is a bar drawn and removed on both settlements; the window is
//! measured against the walk (`DW1084`, §6); the PackTest owes two templates
//! per limit (§8); and `despawn-wave` is the re-seat's own removal (§2).
//!
//! The fixture is hello-world: talk to the keeper, then go through the door
//! he opens. The limit is on the door (`obj/exit`, a `reach-anchor`).

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};
use serde_json::{Value, json};

const NS: &str = "hello-world";

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-limit-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// hello-world with `limit` on `obj/exit` and `patch` applied to the quests
/// document after it.
fn fixture(tag: &str, limit: Value, patch: impl FnOnce(&mut Value)) -> PathBuf {
    let dir = scratch(tag);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    common::patch_file(&dir.join("quests.json"), |v| {
        v["content"]["quests"][0]["objectives"][1]["limit"] = limit;
        patch(v);
    });
    dir
}

/// The window the fixture declares unless a test says otherwise: a minute,
/// with an expiry that tells the party and writes a flag.
fn minute() -> Value {
    json!({
        "ticks": 1200,
        "on_expire": [
            {"type": "narrate", "text": "The keeper bars the door again.",
             "happening": {"text": "the door closes on the party", "verb": "seals"}},
            {"type": "set-flag", "flag": "flag/late",
             "happening": {"text": "the party came too late", "verb": "learns"}}
        ]
    })
}

fn build_result(dir: &Path) -> Result<BuildOutput, BuildFailure> {
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
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn build(dir: &Path) -> BuildOutput {
    build_result(dir).unwrap_or_else(|e| panic!("the fixture builds: {e:?}"))
}

/// A datapack function's text.
fn func(out: &BuildOutput, name: &str) -> String {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    String::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("no function `{name}` in the build"))
            .clone(),
    )
    .unwrap()
}

/// A PackTest template's text.
fn template(out: &BuildOutput, name: &str) -> String {
    let path = format!("packtest-datapack/data/{NS}/test/{name}.mcfunction");
    String::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("no template `{name}` in the build"))
            .clone(),
    )
    .unwrap()
}

/// The index of the first line of `text` containing `needle`.
fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|l| l.contains(needle))
        .unwrap_or_else(|| panic!("no line containing `{needle}` in:\n{text}"))
}

#[test]
fn a_limited_objective_settles_done_or_expired() {
    let dir = fixture("two", minute(), |_| {});
    let out = build(&dir);
    // The clock starts at the objective's own activation, at the declared tick,
    // in replace mode.
    let act = func(&out, "activate_o_exit");
    assert!(
        act.contains(&format!(
            "schedule function {NS}:expire_o_exit 1200t replace"
        )),
        "{act}"
    );
    // Completion settles it and clears the clock.
    let done = func(&out, "complete_o_exit");
    assert!(
        done.contains("scoreboard players set #party dw.os_exit 1"),
        "{done}"
    );
    assert!(
        done.contains(&format!("schedule clear {NS}:expire_o_exit")),
        "a done objective is never expired: its completion clears the clock\n{done}"
    );
    // The expiry settles only a pending objective.
    let exp = func(&out, "expire_o_exit");
    assert!(
        exp.contains("unless score #party dw.o_exit matches 1")
            && exp.contains("unless score #party dw.x_exit matches 1")
            && exp.contains(&format!("run function {NS}:settle_expired_o_exit")),
        "{exp}"
    );
    // The settlement: expired, settled, marked before `on_expire`, told.
    let settle = func(&out, "settle_expired_o_exit");
    assert!(
        settle.contains("scoreboard players set #party dw.x_exit 1"),
        "{settle}"
    );
    assert!(
        settle.contains("scoreboard players set #party dw.os_exit 1"),
        "{settle}"
    );
    let marker = line_of(&settle, &format!("[dw:expired {NS} obj/exit]"));
    let first_effect = line_of(&settle, "dw.f_late");
    assert!(
        marker < first_effect,
        "the marker precedes the first `on_expire` line\n{settle}"
    );
    assert!(
        settle.contains("delvewright.ui.limit.expired"),
        "an announced objective's expiry is told\n{settle}"
    );
    assert!(settle.contains("block.note_block.bass"), "{settle}");
    assert!(
        settle.contains(&format!("function {NS}:check_q_open_the_door")),
        "the quest's completion is checked on expiry\n{settle}"
    );
    // The quest counts the expired objective as settled.
    let check = func(&out, "check_q_open_the_door");
    assert!(
        check.contains("if score #party dw.os_exit matches 1"),
        "{check}"
    );
    // Every driver of the objective reads the expired score.
    let tick = func(&out, "tick");
    let drivers: Vec<&str> = tick
        .lines()
        .filter(|l| l.contains("complete_o_exit") || l.contains("announce_o_exit"))
        .collect();
    assert!(!drivers.is_empty(), "the objective has drivers\n{tick}");
    for d in &drivers {
        assert!(
            d.contains("unless score #party dw.x_exit matches 1"),
            "a driver of a limited objective must be closed by its expiry: {d}"
        );
    }
}

/// An objective `after` a limited one is open once the limited one is
/// settled either way — its `after` term reads the settled score.
#[test]
fn an_objective_after_a_limited_one_opens_on_either_settlement() {
    let dir = fixture("after", minute(), |v| {
        let objs = v["content"]["quests"][0]["objectives"]
            .as_array_mut()
            .unwrap();
        // Limit the talk instead, so the door is the dependent.
        let lim = objs[1].as_object_mut().unwrap().remove("limit").unwrap();
        objs[0]["limit"] = lim;
    });
    let out = build(&dir);
    let tick = func(&out, "tick");
    let exit_driver = tick
        .lines()
        .find(|l| l.contains("run function hello-world:complete_o_exit"))
        .unwrap_or_else(|| panic!("no exit driver in\n{tick}"));
    assert!(
        exit_driver.contains("if score #party dw.os_talk matches 1"),
        "the dependent's `after` term reads the settled score: {exit_driver}"
    );
    // A talk-to's completing dialogue option is closed by its expiry too.
    let any_option_guard = out
        .iter()
        .filter(|(p, _)| p.ends_with(".mcfunction") && p.starts_with("datapack/"))
        .filter_map(|(_, b)| std::str::from_utf8(b).ok())
        .flat_map(str::lines)
        .any(|l| l.contains("dw.x_talk") && l.contains("unless score"));
    assert!(any_option_guard, "the talk's option guard reads its expiry");
}

/// **Perturbation of criterion 2:** without the `schedule clear` in
/// `complete_<obj>`, a done objective's clock is still live and would expire
/// it. The assertion that the line is there is the one that reds.
#[test]
fn a_done_objective_is_never_expired() {
    let dir = fixture("never", minute(), |_| {});
    let out = build(&dir);
    let done = func(&out, "complete_o_exit");
    let set = line_of(&done, "scoreboard players set #party dw.o_exit 1");
    let clear = line_of(&done, &format!("schedule clear {NS}:expire_o_exit"));
    assert!(set < clear || clear < set, "both lines are present");
    // And the expiry itself is refused once the done score is set.
    let exp = func(&out, "expire_o_exit");
    assert!(
        exp.contains("unless score #party dw.o_exit matches 1"),
        "{exp}"
    );
}

#[test]
fn the_three_starts_schedule_where_they_say() {
    // activation: in the objective's own activation.
    let out = build(&fixture("start-act", minute(), |_| {}));
    assert!(func(&out, "activate_o_exit").contains("expire_o_exit 1200t replace"));
    // quest-start: in the quest's activation.
    let mut l = minute();
    l["from"] = json!("quest-start");
    l["ticks"] = json!(2400);
    let out = build(&fixture("start-quest", l, |_| {}));
    let qa = func(&out, "activate_q_open_the_door");
    assert!(qa.contains("expire_o_exit 2400t replace"), "{qa}");
    let tick = func(&out, "tick");
    assert!(
        tick.lines().any(|l| l.starts_with("execute if score #party dw.qa_open_the_door matches 1")
            && l.ends_with(&format!(
                "unless score #actq_open_the_door dw.sys matches 1 run function {NS}:activate_q_open_the_door"
            ))),
        "{tick}"
    );
    // {objective}: in the named objective's activation.
    let mut l = minute();
    l["from"] = json!({"objective": "obj/talk"});
    l["ticks"] = json!(3600);
    let out = build(&fixture("start-obj", l, |_| {}));
    let talk = func(&out, "activate_o_talk");
    assert!(talk.contains("expire_o_exit 3600t replace"), "{talk}");
    assert!(
        !out.contains_key(&format!(
            "datapack/data/{NS}/function/activate_q_open_the_door.mcfunction"
        )),
        "no quest-start clock, no quest activation function"
    );
}

#[test]
fn the_countdown_is_drawn_and_removed_on_both_settlements() {
    let out = build(&fixture("bar", minute(), |_| {}));
    let bar = format!("{NS}:limit_exit");
    let act = func(&out, "activate_o_exit");
    assert!(act.contains(&format!("bossbar add {bar}")), "{act}");
    assert!(act.contains(&format!("bossbar set {bar} max 60")), "{act}");
    assert!(
        act.contains(&format!("bossbar set {bar} players @a")),
        "{act}"
    );
    assert!(
        act.contains(&format!(
            "schedule function {NS}:limit_tick_o_exit 20t replace"
        )),
        "{act}"
    );
    let step = func(&out, "limit_tick_o_exit");
    assert!(
        step.contains(&format!(
            "schedule function {NS}:limit_tick_o_exit 20t replace"
        )),
        "{step}"
    );
    let show = func(&out, "limit_show_o_exit");
    assert!(show.contains("delvewright.ui.limit.countdown"), "{show}");
    assert!(show.contains("Go through the door"), "{show}");
    assert!(show.contains(&format!("bossbar set {bar} name")), "{show}");
    for f in ["complete_o_exit", "settle_expired_o_exit"] {
        let body = func(&out, f);
        assert!(
            body.contains(&format!("bossbar remove {bar}")),
            "{f}: {body}"
        );
        assert!(
            body.contains(&format!("schedule clear {NS}:limit_tick_o_exit")),
            "{f}: {body}"
        );
    }
    for s in [
        delvewright_dsl::chrome::LIMIT_COUNTDOWN,
        delvewright_dsl::chrome::LIMIT_EXPIRED,
    ] {
        assert!(delvewright_dsl::chrome::ALL.contains(&s));
    }
}

/// §8: two templates per limit, the expiry stepping `ticks + 1`.
#[test]
fn the_packtest_owes_two_templates_per_limit() {
    let mut l = minute();
    l["ticks"] = json!(300);
    let out = build(&fixture("pt", l, |_| {}));
    let exp = template(&out, "limit_o_exit_expires");
    assert!(exp.contains("await delay 301t"), "{exp}");
    assert!(
        exp.contains("assert score #party dw.x_exit matches 1"),
        "{exp}"
    );
    assert!(
        exp.contains("#lim_done_exit"),
        "the done score is read: {exp}"
    );
    assert!(
        exp.contains("assert score #party dw.f_late matches 1"),
        "{exp}"
    );
    assert!(
        exp.contains("#lim_open_exit"),
        "the drivers' guard is read: {exp}"
    );
    let done = template(&out, "limit_o_exit_done");
    assert!(done.contains("await delay 301t"), "{done}");
    assert!(
        done.contains("function hello-world:complete_o_exit"),
        "{done}"
    );
    assert!(
        done.contains("assert score #party dw.o_exit matches 1"),
        "{done}"
    );
    assert!(
        done.contains("#lim_x_exit"),
        "the expired score is read: {done}"
    );
}

/// §2: `despawn-wave` is the re-seat's own removal. `souls-bonfire`'s
/// `wave/guards` is re-seated at a rest; the door's completion takes it
/// home, and the lines it emits are the re-seat's, before its spawn.
#[test]
fn despawn_wave_is_the_reseat_removal() {
    let dir = scratch("despawn");
    common::copy_dir_all(&common::compiler_fixtures_dir().join("souls-bonfire"), &dir);
    common::patch_file(&dir.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][1];
        q["on_objective_complete"]["obj/door"]
            .as_array_mut()
            .expect("the door has a bundle")
            .push(json!({
                "type": "despawn-wave", "wave": "wave/guards",
                "happening": {"text": "the guards are taken home", "verb": "departs"}
            }));
    });
    let loaded = load_campaign_dir(&dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("builds");
    let read = |name: &str| {
        String::from_utf8(
            out[&format!("datapack/data/souls-bonfire/function/{name}.mcfunction")].clone(),
        )
        .unwrap()
    };
    let reseat = read("wave_reseat_guards");
    let removal: Vec<&str> = reseat
        .lines()
        .filter(|l| !l.contains("function souls-bonfire:spawn_guards"))
        .collect();
    assert!(!removal.is_empty(), "the re-seat removes: {reseat}");
    let done = read("complete_o_door");
    let lines: Vec<&str> = done.lines().collect();
    let at = lines
        .windows(removal.len())
        .position(|w| w == removal.as_slice())
        .unwrap_or_else(|| {
            panic!("the re-seat's removal is not in the despawn:\n{done}\n---\n{reseat}")
        });
    assert!(at > 0);
}

// ---------------------------------------------------------------------------
// §6 — the window, measured against the walk (`DW1084`).
// ---------------------------------------------------------------------------

fn delvec_build(dir: &Path) -> (i32, String, PathBuf) {
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

fn limit_ledger(out: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(out.join("validation/limit.json")).unwrap())
        .unwrap()
}

/// A window longer than the walk is green and the line states walk, window
/// and percentage; the same window one tick shorter than the walk is
/// `DW1084`, naming the objective, the start cell and both tick counts.
/// The perturbation moves the window alone across the walk it measured.
#[test]
fn a_window_shorter_than_its_walk_is_refused() {
    let (code, log, _) = delvec_build(&fixture("walk-green", minute(), |_| {}));
    assert_eq!(code, 0, "{log}");
    let line = log
        .lines()
        .find(|l| l.starts_with("limit binding (DW1084):"))
        .unwrap_or_else(|| panic!("no binding line:\n{log}"));
    assert!(line.contains("1 objective(s) carry a limit"), "{line}");
    assert!(
        line.contains("of 1200 tick(s)") && line.contains("%) — obj/exit"),
        "{line}"
    );
    // The perturbation is taken from the spawn (`quest-start`), the longer
    // walk, so a window one tick under it is still a legal window.
    let mut l = minute();
    l["from"] = json!("quest-start");
    let (code, log, out) = delvec_build(&fixture("walk-measure", l, |_| {}));
    assert_eq!(code, 0, "{log}");
    let ledger = limit_ledger(&out);
    let walk = ledger["limits"][0]["walk_ticks"].as_u64().expect("walked");
    assert!(
        walk > 20,
        "the door is more than five blocks from the spawn: {ledger}"
    );
    // Exactly the walk: green (the party sprints it exactly).
    let mut l = minute();
    l["from"] = json!("quest-start");
    l["ticks"] = json!(walk);
    let (code, log, _) = delvec_build(&fixture("walk-equal", l, |_| {}));
    assert_eq!(code, 0, "a window the walk equals is green:\n{log}");
    // One tick under: refused.
    let mut l = minute();
    l["from"] = json!("quest-start");
    l["ticks"] = json!(walk - 1);
    let (code, log, _) = delvec_build(&fixture("walk-red", l, |_| {}));
    assert_ne!(code, 0, "{log}");
    assert!(log.contains("DW1084"), "{log}");
    assert!(log.contains("obj/exit"), "{log}");
    assert!(log.contains(&format!("{} tick(s)", walk - 1)), "{log}");
    assert!(log.contains(&format!("{walk} tick(s)")), "{log}");
    assert!(
        log.contains("[5, 65, 2]") || log.contains("step 0"),
        "the start is named: {log}"
    );
}

/// `from: quest-start` measures from the step that opens the quest — the
/// spawn, for hello-world's campaign-start quest — which is a longer walk
/// than from the keeper.
#[test]
fn quest_start_measures_from_the_triggering_step() {
    let (_, log, out) = delvec_build(&fixture("qs-act", minute(), |_| {}));
    let act = limit_ledger(&out)["limits"][0].clone();
    let mut l = minute();
    l["from"] = json!("quest-start");
    let (code, log2, out) = delvec_build(&fixture("qs-quest", l, |_| {}));
    assert_eq!(code, 0, "{log}\n{log2}");
    let qs = limit_ledger(&out)["limits"][0].clone();
    assert!(
        qs["walk_ticks"].as_u64() > act["walk_ticks"].as_u64(),
        "quest-start walks from the spawn, past the keeper: {qs} vs {act}"
    );
    assert_ne!(qs["walk_from"], act["walk_from"], "{qs} vs {act}");
}

/// spec-0133 §3: a clock started before its objective opens — here at the
/// keeper's talk — can run out while the objective is still closed. The clock
/// marks it run out (`#lx_<obj>`), and the objective is **born expired**: the
/// tick it opens it settles expired, `on_expire` firing, never left with no
/// window at all. The old behaviour had neither the mark nor the driver, and
/// left such an objective open forever with no clock.
#[test]
fn an_objective_whose_clock_ran_out_before_it_opened_is_born_expired() {
    let mut l = minute();
    l["from"] = json!({"objective": "obj/talk"});
    let out = build(&fixture("born", l, |_| {}));
    let exp = func(&out, "expire_o_exit");
    assert!(
        exp.contains(
            "execute unless score #party dw.o_exit matches 1 run scoreboard players set #lx_exit dw.sys 1"
        ),
        "the clock marks a not-done objective run out, open or not:\n{exp}"
    );
    let tick = func(&out, "tick");
    let born = tick
        .lines()
        .find(|l| l.starts_with("execute if score #lx_exit dw.sys matches 1"))
        .unwrap_or_else(|| panic!("no born-expired driver in\n{tick}"));
    assert!(
        born.contains("if score #party dw.o_talk matches 1")
            && born.contains("unless score #party dw.x_exit matches 1")
            && born.ends_with(&format!("run function {NS}:settle_expired_o_exit")),
        "it settles the objective the tick its own guard opens, once: {born}"
    );
    assert!(func(&out, "setup").contains("scoreboard players set #lx_exit dw.sys 0"));
    // A clock from the objective's own activation cannot run out before it
    // opens: no driver.
    let out = build(&fixture("born-act", minute(), |_| {}));
    assert!(!func(&out, "tick").contains("#lx_exit dw.sys matches 1"));
}

/// spec-0133 §3: a `quest-start` clock on a quest active at world load starts
/// at the first player's arrival — the first-join hook `join_place` keys on —
/// never at tick 1 of a world nobody has entered.
#[test]
fn a_quest_start_clock_at_world_load_waits_for_the_first_arrival() {
    let mut l = minute();
    l["from"] = json!("quest-start");
    let out = build(&fixture("arrival", l, |_| {}));
    let tick = func(&out, "tick");
    let driver = tick
        .lines()
        .find(|l| l.ends_with(&format!("run function {NS}:activate_q_open_the_door")))
        .unwrap_or_else(|| panic!("no quest-start driver in\n{tick}"));
    let joined = out.contains_key(&format!(
        "datapack/data/{NS}/function/join_place.mcfunction"
    ));
    let arrival = if joined {
        " if entity @a[tag=dw_joined] "
    } else {
        " if entity @a "
    };
    assert!(driver.contains(arrival), "{driver}");
}
