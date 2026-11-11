//! spec-0134 §7 — a quorum of objectives. `after` admits `{any, at_least}`:
//! the objective waiting on it opens when `at_least` of the members are done;
//! the moment the quorum holds, every member not yet done is **lapsed** —
//! settled, its completion bundle never fired, its act refused after, its
//! announcement closed with `limit.lapsed`, counted toward its quest.
//!
//! The fixture is hello-world: talk to the keeper, then three places of the
//! room, any two of which open the door.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::flow::{Flow, WorldOrigin};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, Diagnostic, parse_campaign, validate_campaign_with};
use serde_json::{Value, json};

const NS: &str = "hello-world";

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("quorum-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn member(id: &str, anchor: &str, title: &str) -> Value {
    json!({
        "type": "reach-anchor", "id": id, "anchor": anchor, "radius": 1,
        "after": ["obj/talk"], "title": title,
        "hint": "Somewhere in the keeper's room.",
        "happening": {"text": format!("the party completes {id}"), "verb": "arrives"}
    })
}

/// hello-world with three members after the talk and the door after any two,
/// `quorum` standing for the door's whole `after` term; `patch` after it.
fn fixture(tag: &str, quorum: Value, patch: impl FnOnce(&mut Value)) -> PathBuf {
    let dir = scratch(tag);
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    common::patch_file(&dir.join("quests.json"), |v| {
        let objs = v["content"]["quests"][0]["objectives"]
            .as_array_mut()
            .unwrap();
        objs.insert(1, member("obj/see-the-hearth", "spawn", "See the hearth"));
        objs.insert(
            2,
            member("obj/see-the-stand", "anchor/keeper-stand", "See the stand"),
        );
        objs.insert(3, member("obj/see-the-sill", "anchor/exit", "See the sill"));
        objs[4]["after"] = json!(["obj/talk", quorum]);
        v["content"]["quests"][0]["on_objective_complete"]["obj/see-the-sill"] = json!([
            {"type": "narrate", "text": "The sill is cold.",
             "happening": {"text": "the party learns the sill is cold", "verb": "learns"}}
        ]);
        patch(v);
    });
    dir
}

fn two_of_three() -> Value {
    json!({"any": ["obj/see-the-hearth", "obj/see-the-stand", "obj/see-the-sill"], "at_least": 2})
}

fn campaign(dir: &Path) -> Campaign {
    parse_campaign(&load_campaign_dir(dir).unwrap().raw).expect("fixture parses")
}

fn diags(dir: &Path) -> Vec<Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        &campaign(dir),
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

fn errors(dir: &Path) -> Vec<(String, String)> {
    diags(dir)
        .into_iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .map(|d| (d.code.to_string(), d.path))
        .collect()
}

fn build(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let c = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
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
    .unwrap_or_else(|e| panic!("the fixture builds: {e:?}"))
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

#[test]
fn a_quorum_validates() {
    let dir = fixture("valid", two_of_three(), |_| {});
    assert!(errors(&dir).is_empty(), "{:#?}", errors(&dir));
}

/// The guard sums the members and compares: every member's completion
/// re-sums the done scores into the quorum's count and lapses at `at_least`;
/// the door's every guard reads the count.
#[test]
fn the_guard_sums_the_members_and_compares() {
    let dir = fixture("guard", two_of_three(), |_| {});
    let out = build(&dir);
    let hearth = func(&out, "complete_o_see_the_hearth");
    for line in [
        "scoreboard players set #q_exit_0 dw.sys 0",
        "execute if score #party dw.o_see_the_hearth matches 1 run scoreboard players add #q_exit_0 dw.sys 1",
        "execute if score #party dw.o_see_the_stand matches 1 run scoreboard players add #q_exit_0 dw.sys 1",
        "execute if score #party dw.o_see_the_sill matches 1 run scoreboard players add #q_exit_0 dw.sys 1",
        "execute if score #q_exit_0 dw.sys matches 2.. run function hello-world:lapse_q_o_exit_0",
    ] {
        assert!(hearth.lines().any(|l| l == line), "`{line}` in:\n{hearth}");
    }
    let all: String = out
        .iter()
        .filter(|(k, _)| k.ends_with(".mcfunction"))
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .collect();
    assert!(
        all.lines().any(|l| l.contains("if score #q_exit_0 dw.sys matches 2..")
            && l.contains("dw.o_exit")),
        "the door's guard reads the count"
    );
}

/// The members not done when the quorum holds are lapsed: settled, closed
/// with the lapse line, their affordance and highlight gone, their quest
/// asked — and every guard of a member refuses its act once it is settled.
#[test]
fn the_members_not_done_are_lapsed_with_the_line_and_the_guard() {
    let dir = fixture("lapse", two_of_three(), |_| {});
    let out = build(&dir);
    let lapse = func(&out, "lapse_q_o_exit_0");
    for m in ["see_the_hearth", "see_the_stand", "see_the_sill"] {
        assert!(
            lapse.contains(&format!(
                "execute unless score #party dw.os_{m} matches 1 run function hello-world:settle_lapsed_o_{m}"
            )),
            "{lapse}"
        );
    }
    let settle = func(&out, "settle_lapsed_o_see_the_sill");
    assert!(
        settle.starts_with("scoreboard players set #party dw.os_see_the_sill 1"),
        "{settle}"
    );
    assert!(settle.contains("delvewright.ui.limit.lapsed"), "{settle}");
    assert!(
        settle.contains("function hello-world:check_q_open_the_door"),
        "{settle}"
    );
    // The bundle of the member's completion is never in its settlement.
    assert!(!settle.contains("The sill is cold."), "{settle}");
    // A member's guards refuse once it is settled; the quest counts it settled.
    let all: String = out
        .iter()
        .filter(|(k, _)| k.ends_with(".mcfunction"))
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .collect();
    assert!(
        all.lines().any(
            |l| l.contains("unless score #party dw.o_see_the_sill matches 1")
                && l.contains("unless score #party dw.os_see_the_sill matches 1")
        ),
        "the sill's pending guard refuses it lapsed"
    );
    let check = func(&out, "check_q_open_the_door");
    assert!(
        check.contains("if score #party dw.os_see_the_sill matches 1"),
        "{check}"
    );
    // A member's completion settles it.
    assert!(
        func(&out, "complete_o_see_the_sill")
            .contains("scoreboard players set #party dw.os_see_the_sill 1")
    );
}

/// A quorum of two of three enumerates three witnesses: the all world (the
/// first two by id) and one world per other pair; each world's path walks its
/// pair and never the lapsed third.
#[test]
fn two_of_three_enumerates_three_witnesses() {
    let dir = fixture("worlds", two_of_three(), |_| {});
    let c = campaign(&dir);
    let flow = Flow::new(&c);
    let story = flow.story();
    let quorum_worlds: Vec<usize> = story
        .worlds
        .iter()
        .enumerate()
        .filter(|(_, w)| w.origin == WorldOrigin::Quorum)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(quorum_worlds.len(), 2, "{:#?}", story.worlds);
    let members = [
        "obj/see-the-hearth",
        "obj/see-the-stand",
        "obj/see-the-sill",
    ];
    let walked = |k: usize| -> Vec<&str> {
        let p = flow.story_playthrough(k);
        members
            .iter()
            .copied()
            .filter(|m| p.steps.iter().any(|s| s.objective == *m))
            .collect()
    };
    // The all world: the first two members by id order.
    assert_eq!(walked(0), vec!["obj/see-the-hearth", "obj/see-the-sill"]);
    let mut pairs: Vec<Vec<&str>> = vec![walked(0)];
    pairs.extend(quorum_worlds.iter().map(|&k| walked(k)));
    pairs.sort();
    assert_eq!(
        pairs,
        vec![
            vec!["obj/see-the-hearth", "obj/see-the-sill"],
            vec!["obj/see-the-hearth", "obj/see-the-stand"],
            vec!["obj/see-the-stand", "obj/see-the-sill"],
        ]
    );
    for k in 0..flow.story_count() {
        assert!(
            flow.story_playthrough(k)
                .steps
                .iter()
                .any(|s| s.objective == "obj/exit"),
            "world {k} reaches the door"
        );
    }
}

#[test]
fn a_quorum_of_every_member_or_a_repeated_one_is_dw0100() {
    let all = json!({"any": ["obj/see-the-hearth", "obj/see-the-stand"], "at_least": 2});
    let dir = fixture("all", all, |_| {});
    assert!(
        errors(&dir).contains(&(
            "DW0100".to_string(),
            "/content/quests/0/objectives/4/after/1/at_least".to_string()
        )),
        "{:#?}",
        errors(&dir)
    );
    let twice = json!({"any": ["obj/see-the-hearth", "obj/see-the-hearth", "obj/see-the-stand"], "at_least": 1});
    let dir = fixture("twice", twice, |_| {});
    assert!(
        errors(&dir).contains(&(
            "DW0100".to_string(),
            "/content/quests/0/objectives/4/after/1/any/1".to_string()
        )),
        "{:#?}",
        errors(&dir)
    );
}

#[test]
fn a_cycle_through_a_member_is_dw0140() {
    let dir = fixture("cycle", two_of_three(), |v| {
        v["content"]["quests"][0]["objectives"][1]["after"] = json!(["obj/talk", "obj/exit"]);
    });
    assert!(
        errors(&dir).iter().any(|(c, _)| c == "DW0140"),
        "{:#?}",
        errors(&dir)
    );
}

/// The guard every reader writes and the one definition agree: over every
/// state of the three members (none, done or lapsed) and the talk, the emitted
/// door guard's `after` terms, read as a scoreboard would, hold exactly when
/// `Objective::after_holds` says they do.
#[test]
fn the_emitted_guard_and_the_one_definition_agree() {
    let dir = fixture("agree", two_of_three(), |_| {});
    let c = campaign(&dir);
    let door = c.quests.content.quests[0].objectives[4].clone();
    let out = build(&dir);
    let all: String = out
        .iter()
        .filter(|(k, _)| k.ends_with(".mcfunction"))
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .collect();
    // The door's activation guard: its `if` terms over party and `dw.sys`.
    let guard = all
        .lines()
        .find(|l| {
            l.contains("if score #q_exit_0 dw.sys matches ")
                && l.contains("unless score #party dw.o_exit matches 1")
        })
        .expect("the door's pending guard")
        .to_string();
    let members = ["see_the_hearth", "see_the_stand", "see_the_sill"];
    for talk in [false, true] {
        for code in 0..27u32 {
            // Each member: 0 pending, 1 done, 2 lapsed.
            let st: Vec<u32> = (0..3).map(|i| (code / 3u32.pow(i)) % 3).collect();
            let done = |id: &str| {
                id == "obj/talk" && talk
                    || members
                        .iter()
                        .zip(&st)
                        .any(|(m, s)| id == format!("obj/{}", m.replace('_', "-")) && *s == 1)
            };
            let settled = |id: &str| {
                done(id)
                    || members
                        .iter()
                        .zip(&st)
                        .any(|(m, s)| id == format!("obj/{}", m.replace('_', "-")) && *s == 2)
            };
            let want = door.after_holds(&settled, &done);
            // The scoreboard: done scores, settled scores, and the summed count.
            let mut score: BTreeMap<String, i64> = BTreeMap::new();
            score.insert("#party dw.o_talk".into(), talk as i64);
            let mut count = 0;
            for (m, s) in members.iter().zip(&st) {
                score.insert(format!("#party dw.o_{m}"), (*s == 1) as i64);
                score.insert(format!("#party dw.os_{m}"), (*s >= 1) as i64);
                count += (*s == 1) as i64;
            }
            score.insert("#q_exit_0 dw.sys".into(), count);
            let words: Vec<&str> = guard.split_whitespace().collect();
            let mut got = true;
            let mut i = 0;
            while i + 5 < words.len() {
                if words[i] == "if" && words[i + 1] == "score" && words[i + 4] == "matches" {
                    let key = format!("{} {}", words[i + 2], words[i + 3]);
                    let is_after = key.starts_with("#q_") || key == "#party dw.o_talk";
                    if is_after {
                        let v = score.get(&key).copied().unwrap_or(0);
                        let range = words[i + 5];
                        let ok = match range.strip_suffix("..") {
                            Some(lo) => v >= lo.parse::<i64>().unwrap(),
                            None => v == range.parse::<i64>().unwrap(),
                        };
                        got &= ok;
                    }
                    i += 6;
                } else {
                    i += 1;
                }
            }
            assert_eq!(got, want, "talk {talk}, members {st:?}: `{guard}`");
        }
    }
}
