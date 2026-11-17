//! A completion runs once, whatever the party's size. Every caller of
//! `complete_<obj>` is a fork over the players present, and the pinned game
//! reads a fork's conditions for every player before it runs the function for
//! any of them, so the caller's own not-done guard admits each player. The
//! body's own first line is the latch, and a completion that seats a wave owes
//! a PackTest that completes it under a three-player fork and counts the
//! bodies. Proven on emitted bytes of the souls-bonfire fixture, whose
//! `obj/talk` seats `wave/guards` (two zombies) with no envelope.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{MobCount, ScaledCount, parse_campaign};

const NS: &str = "souls-bonfire";

fn build_with(mutate: impl FnOnce(&mut delvewright_dsl::Campaign)) -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join(NS);
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    mutate(&mut campaign);
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
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

fn template(out: &BuildOutput, obj: &str) -> Option<String> {
    out.get(&format!(
        "packtest-datapack/data/{NS}/test/party_seat_once_{obj}.mcfunction"
    ))
    .map(|b| String::from_utf8(b.clone()).unwrap())
}

/// Every `complete_<obj>` the build emits, by name.
fn completions(out: &BuildOutput) -> Vec<(String, String)> {
    let dir = format!("datapack/data/{NS}/function/complete_o_");
    out.iter()
        .filter_map(|(k, v)| {
            let name = k.strip_prefix(&dir)?.strip_suffix(".mcfunction")?;
            Some((name.to_string(), String::from_utf8(v.clone()).unwrap()))
        })
        .collect()
}

#[test]
fn every_completion_returns_first_when_its_objective_is_already_done() {
    let out = build_with(|_| {});
    let all = completions(&out);
    // Every stage-5 objective of the fixture emits its completion.
    assert_eq!(
        all.len(),
        5,
        "{:?}",
        all.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
    for (name, body) in &all {
        let first = body.lines().next().unwrap_or_default();
        assert_eq!(
            first,
            format!("execute if score #party dw.o_{name} matches 1 run return 0"),
            "complete_o_{name} must open on its own latch:\n{body}"
        );
    }
}

#[test]
fn a_completion_that_seats_a_wave_is_driven_under_a_three_player_fork() {
    let out = build_with(|_| {});
    let t = template(&out, "talk").expect("obj/talk seats wave/guards: its template is owed");
    let lines: Vec<&str> = t.lines().collect();
    // Alone, so `@a` is the template's own three players.
    assert!(
        lines.contains(&format!("# @environment {NS}:solo/party_seat_once_talk").as_str()),
        "{t}"
    );
    assert_eq!(
        lines
            .iter()
            .filter(|l| l.starts_with("dummy ") && l.ends_with(" spawn"))
            .count(),
        2,
        "{t}"
    );
    let open = lines
        .iter()
        .position(|l| *l == "scoreboard players set #party dw.o_talk 0")
        .expect("the objective made not-done");
    let drive = lines
        .iter()
        .position(|l| *l == format!("execute as @a run function {NS}:complete_o_talk"))
        .expect("the completion driven under a fork over every player");
    assert!(open < drive, "{t}");
    // Two zombies, whatever the party: one seating.
    assert!(
        lines.contains(&"assert score #pso_talk_0 dw.sys matches 2"),
        "{t}"
    );
    // `obj/door` seats `wave/ambush` (two sheep) with no envelope too.
    let door = template(&out, "door").expect("obj/door seats wave/ambush");
    assert!(
        door.contains("assert score #pso_door_0 dw.sys matches 2"),
        "{door}"
    );
}

#[test]
fn a_scaled_wave_is_counted_at_the_three_players_present() {
    let out = build_with(|c| {
        c.quests.content.waves[0].mobs[0].count = MobCount::Scaled(ScaledCount {
            base: 2,
            per_player: 1,
        });
    });
    let t = template(&out, "talk").unwrap();
    assert!(
        t.contains("assert score #pso_talk_0 dw.sys matches 4"),
        "{t}"
    );
}

#[test]
fn a_seat_under_a_when_is_not_driven() {
    let out = build_with(|c| {
        let q = &mut c.quests.content.quests[0];
        let bundle = q
            .on_objective_complete
            .get_mut(&delvewright_dsl::ObjectiveId("obj/talk".into()))
            .unwrap();
        for e in bundle.iter_mut() {
            if matches!(e.verb, delvewright_dsl::Verb::SpawnWave { .. }) {
                e.when = Some(delvewright_dsl::Guard {
                    requires_flags: vec![delvewright_dsl::FlagId("flag/summoned".into())],
                    ..Default::default()
                });
            }
        }
    });
    assert!(template(&out, "talk").is_none());
    assert!(template(&out, "door").is_some());
}
