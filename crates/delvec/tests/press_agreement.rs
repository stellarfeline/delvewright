//! **One press, one dispatch** (spec-0117 §5.2): the agreement test.
//!
//! Every pressable's state function `pst_<owner>`, its answers' dispatches
//! `press_dw_answer_<owner>_<state>` and its act — an `interact`'s completion
//! line in `tick`, a trigger's `press_<id>` — are each built from the owner's own
//! gate authorities (`pending_guard`, `trigger_poll_guards`). This test reads
//! them back off the gallery's shipped functions and evaluates them, over every
//! assignment of the scores they read, as the server would for one press: under
//! every assignment **exactly one** of {the act, `not_yet`, `missing_item`,
//! `done`} dispatches. A guard restated anywhere, or a state the answers leave
//! uncovered, reds here with the assignment that shows it.
//!
//! The evaluator reads only the clause kinds these functions are built of
//! (`as`, `if|unless score … matches`, `if|unless items entity @s
//! weapon.mainhand`, `if|unless entity @s[tag=…]`, `run`) and refuses any other,
//! so a new clause kind cannot be skipped in silence.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A score range as `matches` writes it: `N`, `..N`, `N..`, `A..B`.
fn in_range(v: i64, r: &str) -> bool {
    match r.split_once("..") {
        None => v == r.parse::<i64>().unwrap(),
        Some((a, b)) => {
            (a.is_empty() || v >= a.parse::<i64>().unwrap())
                && (b.is_empty() || v <= b.parse::<i64>().unwrap())
        }
    }
}

/// The values worth trying for a score a range reads: either side of every
/// boundary.
fn probes(r: &str) -> Vec<i64> {
    let mut v = Vec::new();
    for b in r.split("..").filter(|s| !s.is_empty()) {
        let n: i64 = b.parse().unwrap();
        v.extend([n - 1, n, n + 1]);
    }
    v
}

#[derive(Clone, Default)]
struct World {
    scores: BTreeMap<String, i64>,
    held: bool,
    tags: BTreeSet<String>,
}

/// Whether an `execute` line's condition holds in `w`; `None` for a line that is
/// not an `execute`. Panics on a clause kind it does not read.
fn holds(line: &str, w: &World) -> Option<bool> {
    let rest = line.strip_prefix("execute ")?;
    let t: Vec<&str> = rest.split_whitespace().collect();
    let mut i = 0;
    let mut ok = true;
    while i < t.len() {
        match t[i] {
            "run" => return Some(ok),
            "as" | "at" => i += 2,
            "if" | "unless" => {
                let want = t[i] == "if";
                let got = match t[i + 1] {
                    "score" => {
                        assert_eq!(t[i + 4], "matches", "{line}");
                        let key = format!("{} {}", t[i + 2], t[i + 3]);
                        let v = *w.scores.get(&key).unwrap_or(&0);
                        let r = in_range(v, t[i + 5]);
                        i += 6;
                        r
                    }
                    "items" => {
                        assert_eq!(
                            &t[i + 2..i + 5],
                            ["entity", "@s", "weapon.mainhand"],
                            "{line}"
                        );
                        i += 6;
                        w.held
                    }
                    "entity" => {
                        let sel = t[i + 2];
                        let tag = sel
                            .strip_prefix("@s[tag=")
                            .and_then(|s| s.strip_suffix(']'))
                            .unwrap_or_else(|| {
                                panic!("an entity clause this test does not read: {line}")
                            });
                        i += 3;
                        w.tags.contains(tag)
                    }
                    other => panic!("a clause this test does not read (`{other}`): {line}"),
                };
                ok &= got == want;
            }
            other => panic!("a clause this test does not read (`{other}`): {line}"),
        }
    }
    panic!("an execute with no run: {line}")
}

/// Every score `if|unless score` reads in `lines`, with the values to try.
fn variables(lines: &[&str]) -> BTreeMap<String, BTreeSet<i64>> {
    let mut out: BTreeMap<String, BTreeSet<i64>> = BTreeMap::new();
    for l in lines {
        let t: Vec<&str> = l.split_whitespace().collect();
        for k in 0..t.len().saturating_sub(5) {
            if (t[k] == "if" || t[k] == "unless") && t[k + 1] == "score" && t[k + 4] == "matches" {
                let key = format!("{} {}", t[k + 2], t[k + 3]);
                if key.starts_with("#pst_") {
                    continue;
                }
                out.entry(key).or_default().extend(probes(t[k + 5]));
            }
        }
    }
    out
}

/// Run a `pst_<owner>` body in `w`: each line sets `#pst_<owner>`.
fn run_pst(body: &str, w: &mut World) {
    for l in body.lines() {
        let (set, cond) = match holds(l, w) {
            Some(c) => (l.rsplit_once(" run ").unwrap().1, c),
            None => (l, true),
        };
        if !cond {
            continue;
        }
        let t: Vec<&str> = set.split_whitespace().collect();
        assert_eq!(&t[..3], ["scoreboard", "players", "set"], "{l}");
        w.scores
            .insert(format!("{} {}", t[3], t[4]), t[5].parse().unwrap());
    }
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(format!("{name}.mcfunction")))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The gallery's primary, built once per process: its output directory.
fn gallery() -> &'static Path {
    static OUT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    OUT.get_or_init(|| {
        let dir = tmp("press-agreement");
        let (campaign, prefabs) = common::gallery_primary_at(&dir.join("gallery"));
        let out = dir.join("build");
        let r = Command::new(BIN)
            .arg("build")
            .arg(&campaign)
            .arg("-o")
            .arg(&out)
            .arg("--prefabs")
            .arg(&prefabs)
            .output()
            .unwrap();
        assert!(r.status.success(), "{}", String::from_utf8_lossy(&r.stderr));
        out
    })
}

fn advancement(name: &str) -> serde_json::Value {
    let p = gallery().join(format!("datapack/data/gallery/advancement/{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{name}: {e}")))
        .unwrap()
}

#[test]
fn every_press_dispatches_exactly_one_thing_in_every_state() {
    let out = gallery();
    let f = out.join("datapack/data/gallery/function");
    let tick = read(&f, "tick");
    let mut owners: Vec<String> = std::fs::read_dir(&f)
        .unwrap()
        .filter_map(|e| {
            let n = e.unwrap().file_name().into_string().unwrap();
            n.strip_prefix("pst_")?
                .strip_suffix(".mcfunction")
                .map(str::to_string)
        })
        .collect();
    owners.sort();
    let mut judged = 0usize;
    let mut assignments = 0usize;
    for owner in &owners {
        // The act: an interact's completion line, or a trigger's press dispatch.
        let act: Vec<String> = if let Some(obj) = owner.strip_prefix("obj_") {
            tick.lines()
                .filter(|l| {
                    l.ends_with(&format!(":complete_o_{obj}")) && l.contains("scores={dw.i_")
                })
                .map(str::to_string)
                .collect()
        } else if let Some(trig) = owner.strip_prefix("trig_") {
            match std::fs::read_to_string(f.join(format!("press_{trig}.mcfunction"))) {
                Ok(b) => b
                    .lines()
                    .filter(|l| l.contains(&format!(":trig_{trig}")))
                    .map(str::to_string)
                    .collect(),
                Err(_) => continue,
            }
        } else {
            continue;
        };
        if act.is_empty() {
            continue;
        }
        let pst = read(&f, &format!("pst_{owner}"));
        let answers: Vec<(String, String)> = ["not_yet", "missing_item", "done"]
            .iter()
            .filter_map(|s| {
                let b = std::fs::read_to_string(
                    f.join(format!("press_dw_answer_{owner}_{s}.mcfunction")),
                )
                .ok()?;
                let line = b
                    .lines()
                    .find(|l| l.contains(&format!(":trig_dw_answer_{owner}_{s}")))?
                    .to_string();
                Some((s.to_string(), line))
            })
            .collect();
        let mut all: Vec<&str> = pst.lines().collect();
        all.extend(act.iter().map(String::as_str));
        all.extend(answers.iter().map(|(_, l)| l.as_str()));
        let vars: Vec<(String, Vec<i64>)> = variables(&all)
            .into_iter()
            .map(|(k, v)| (k, v.into_iter().collect()))
            .collect();
        let held_matters = all.iter().any(|l| l.contains("weapon.mainhand"));
        let mut idx = vec![0usize; vars.len()];
        loop {
            for held in [false, true] {
                if held && !held_matters {
                    continue;
                }
                let mut w = World {
                    held,
                    ..World::default()
                };
                for (k, (name, vals)) in vars.iter().enumerate() {
                    w.scores.insert(name.clone(), vals[idx[k]]);
                }
                run_pst(&pst, &mut w);
                // The act runs first and tags the presser; the answers read the tag.
                let acted = act.iter().any(|l| holds(l, &w).unwrap());
                if acted {
                    w.tags.insert(format!("dw_acted_{owner}"));
                }
                let fired: Vec<&str> = answers
                    .iter()
                    .filter(|(_, l)| holds(l, &w).unwrap())
                    .map(|(s, _)| s.as_str())
                    .collect();
                let count = usize::from(acted) + fired.len();
                assert!(
                    count <= 1,
                    "`{owner}`: a press dispatches {count} things (act {acted}, answers {fired:?}) \
                     under {:?}, held {held}",
                    w.scores
                );
                // Where the press's state owes a dispatch — the act while open, the
                // state's answer where one is declared — exactly one happens.
                let has_state = |s: &str| answers.iter().any(|(a, _)| a == s);
                let p = w.scores[&format!("#pst_{owner} dw.sys")];
                let owed = match p {
                    0 => has_state("not_yet"),
                    1 => true,
                    _ => has_state("done"),
                };
                if owed {
                    assert_eq!(
                        count, 1,
                        "`{owner}`: a press in state {p} dispatches nothing under {:?}, held {held}",
                        w.scores
                    );
                }
                assignments += 1;
            }
            // Next assignment.
            let mut k = 0;
            while k < idx.len() {
                idx[k] += 1;
                if idx[k] < vars[k].1.len() {
                    break;
                }
                idx[k] = 0;
                k += 1;
            }
            if k == idx.len() {
                break;
            }
        }
        judged += 1;
    }
    eprintln!(
        "press agreement: {judged} pressable(s) of {} with a state function judged over {assignments} assignment(s)",
        owners.len()
    );
    assert!(
        judged >= 8,
        "the gallery's interacts and gated triggers are judged: {judged}"
    );
}

/// Criterion 9: every synthesized answer plays the refusal cue, a sound the
/// pinned registry has, and the cue ledger states it.
#[test]
fn every_answer_plays_the_refusal_cue() {
    let f = gallery().join("datapack/data/gallery/function");
    let mut answers = 0;
    for e in std::fs::read_dir(&f).unwrap() {
        let p = e.unwrap().path();
        let n = p.file_name().unwrap().to_string_lossy().into_owned();
        if !n.starts_with("trig_dw_answer_") {
            continue;
        }
        let body = std::fs::read_to_string(&p).unwrap();
        // A body's own answers (spec-0119) carry their own cue.
        if n.starts_with("trig_dw_answer_npc_") || n.starts_with("trig_dw_answer_actor_") {
            continue;
        }
        assert!(
            body.contains("playsound minecraft:block.chest.locked player @s"),
            "`{n}` plays the refusal cue: {body}"
        );
        answers += 1;
    }
    assert!(answers >= 20, "the gallery's answers are read: {answers}");
    let sounds =
        std::fs::read_to_string(common::repo_root().join("crates/delvec/data/sounds-1.21.11.json"))
            .unwrap();
    assert!(
        sounds.contains("\"block.chest.locked\"")
            || sounds.contains("minecraft:block.chest.locked")
    );
    let cues: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(gallery().join("validation/cues.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(cues["cues"]["refused"], "minecraft:block.chest.locked");
}

/// Criterion 19: the released bay door answers nothing once open — no `done`
/// answer is emitted for it — and the unreleased chest emits one.
#[test]
fn a_released_door_answers_nothing_once_open_and_the_chest_does() {
    let f = gallery().join("datapack/data/gallery/function");
    assert!(
        !f.join("trig_dw_answer_obj_open_the_bay_door_done.mcfunction")
            .exists()
    );
    assert!(
        f.join("trig_dw_answer_obj_lift_the_lid_done.mcfunction")
            .exists()
    );
    let complete =
        std::fs::read_to_string(f.join("complete_o_open_the_bay_door.mcfunction")).unwrap();
    assert!(
        !complete.contains("summon minecraft:interaction"),
        "the released door's box does not return: {complete}"
    );
}

/// **A door prop is laid as one door** (spec-0117 with spec-0123's
/// `region_fill`): the bay door's two cells hold its lower half under its
/// upper half in the shipped `setup_finish`, and the world the build judges at
/// load holds the same — the gallery builds, and `DW1002` refuses a lower half
/// standing on a lower half, which is what the load model laid when each half
/// was its own one-cell write and `region_fill` read every one-cell region as
/// the door's bottom course.
#[test]
fn a_door_prop_is_laid_as_one_door() {
    let setup = std::fs::read_to_string(
        gallery().join("datapack/data/gallery/function/setup_finish.mcfunction"),
    )
    .unwrap();
    let door: Vec<&str> = setup
        .lines()
        .filter(|l| l.starts_with("setblock ") && l.contains("minecraft:spruce_door"))
        .collect();
    assert_eq!(door.len(), 2, "the bay door's two halves: {door:?}");
    let at = |l: &str| -> Vec<i32> {
        l.split_whitespace()
            .skip(1)
            .take(3)
            .map(|v| v.parse().unwrap())
            .collect()
    };
    let (lower, upper) = (door[0], door[1]);
    assert!(lower.contains("half=lower"), "{lower}");
    assert!(upper.contains("half=upper"), "{upper}");
    let (l, u) = (at(lower), at(upper));
    assert_eq!((u[0], u[1], u[2]), (l[0], l[1] + 1, l[2]), "{door:?}");
}

/// Criterion 20: a `presser` strike's dispatch and its `done` answer are each
/// `player_hurt_entity` on the struck box's tag; a `party` `strike-assembly`
/// emits no dispatch advancement of its own.
#[test]
fn the_blow_names_its_striker() {
    let setup = std::fs::read_to_string(
        gallery().join("datapack/data/gallery/function/setup_finish.mcfunction"),
    )
    .unwrap();
    let hearth_box = setup
        .lines()
        .find(|l| {
            l.contains("summon minecraft:interaction")
                && l.contains("\"dw_trig_strike_the_hearth\"")
        })
        .expect("the hearth stone's box")
        .to_string();
    for (name, tag) in [
        ("press_strike_the_hearth", "dw_trig_strike_the_hearth"),
        (
            "press_dw_answer_trig_strike_the_hearth_done",
            "dw_trig_dw_answer_trig_strike_the_hearth_done",
        ),
    ] {
        let a = advancement(name);
        assert_eq!(
            a["criteria"]["interact"]["trigger"], "minecraft:player_hurt_entity",
            "{name}: {a}"
        );
        assert!(
            a["criteria"]["interact"]["conditions"]["entity"]["nbt"]
                .as_str()
                .unwrap()
                .contains(tag),
            "{name}: {a}"
        );
        assert!(
            hearth_box.contains(&format!("\"{tag}\"")),
            "the struck box carries `{tag}`: {hearth_box}"
        );
    }
    assert!(
        !gallery()
            .join("datapack/data/gallery/advancement/press_strike_the_sentinel.json")
            .exists(),
        "a party strike-assembly keeps its poll"
    );
    let tick =
        std::fs::read_to_string(gallery().join("datapack/data/gallery/function/tick.mcfunction"))
            .unwrap();
    assert!(
        !tick.contains("run function gallery:trig_strike_the_hearth"),
        "a presser strike has no fire clause in tick"
    );
}
