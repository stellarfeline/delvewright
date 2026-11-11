//! spec-0134 §4 — a wave scales with the party. A stack's `count` takes
//! `{base, per_player}`: one player meets `base`, each further player present
//! at the spawn adds `per_player`. Proven on emitted bytes of the
//! souls-bonfire fixture: the spawn counts the players, summons `base` bodies
//! always and each further body on its own proven cell behind a party-size
//! guard (`DW0312` seats the four-player maximum), the countdown starts at
//! `base` and each guarded body adds itself; a name on a stack whose maximum
//! is more than one body is `DW0983`; the critical path's `kill` step (what the
//! harness's muster and countdown read) expects `base`.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{MobCount, ScaledCount, parse_campaign};

fn build_with(mutate: impl FnOnce(&mut delvewright_dsl::Campaign)) -> BuildOutput {
    let dir = common::compiler_fixtures_dir().join("souls-bonfire");
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

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(out.get(path).unwrap_or_else(|| panic!("no {path}")).clone()).unwrap()
}

/// The wave's spawn: its own function and, for a wave that scales, the
/// seating it calls at the size it measured — in the order they run.
fn spawn(out: &BuildOutput) -> String {
    let own = text(
        out,
        "datapack/data/souls-bonfire/function/spawn_guards.mcfunction",
    );
    match out.get("datapack/data/souls-bonfire/function/spawn_guards_party.mcfunction") {
        Some(party) => format!("{own}{}", String::from_utf8(party.clone()).unwrap()),
        None => own,
    }
}

fn two_plus_one() -> MobCount {
    MobCount::Scaled(ScaledCount {
        base: 2,
        per_player: 1,
    })
}

#[test]
fn a_wave_for_the_party_counts_the_players_and_guards_each_further_body() {
    let out = build_with(|c| c.quests.content.waves[0].mobs[0].count = two_plus_one());
    let s = spawn(&out);
    let lines: Vec<&str> = s.lines().collect();
    let count_at = lines
        .iter()
        .position(|l| {
            *l == "execute store result score #pn_guards dw.sys if entity @a[tag=!dw_cutscene]"
        })
        .unwrap_or_else(|| panic!("the party is counted:\n{s}"));
    let summons: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|l| l.contains("summon minecraft:zombie"))
        .collect();
    assert_eq!(summons.len(), 5, "five bodies at four players:\n{s}");
    assert!(summons[0].starts_with("summon ") && summons[1].starts_with("summon "));
    for (k, need) in [(2, 2), (3, 3), (4, 4)] {
        assert!(
            summons[k].starts_with(&format!(
                "execute if score #pn_guards dw.sys matches {need}.. run summon "
            )),
            "body {k} needs {need} players: {}",
            summons[k]
        );
    }
    // Five distinct proven cells (`DW0312` seated the maximum).
    let cells: std::collections::BTreeSet<String> = summons
        .iter()
        .map(|l| {
            let at = l.find("summon minecraft:zombie ").unwrap() + "summon minecraft:zombie ".len();
            l[at..].split(" {").next().unwrap().to_string()
        })
        .collect();
    assert_eq!(cells.len(), 5, "{cells:#?}");
    // The countdown starts at base and each guarded body adds itself.
    let set = lines
        .iter()
        .find(|l| {
            l.starts_with("scoreboard players set ") && l.ends_with(" 2") && l.contains("dw.w")
        })
        .unwrap_or_else(|| panic!("the countdown starts at base:\n{s}"));
    let holder = set.split(' ').nth(3).unwrap();
    let objective = set.split(' ').nth(4).unwrap();
    let adds = lines
        .iter()
        .filter(|l| {
            l.starts_with("execute if score #pn_guards dw.sys matches ")
                && l.ends_with(&format!(
                    "run scoreboard players add {holder} {objective} 1"
                ))
        })
        .count();
    assert_eq!(adds, 3, "{s}");
    assert!(count_at < lines.iter().position(|l| l == set).unwrap());
    // The kill step expects one player's count.
    let cp: serde_json::Value = serde_json::from_slice(&out["critical-path.json"]).unwrap();
    let kill = cp["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| {
            s.as_object()
                .is_some_and(|o| o.values().any(|v| v == "wave/guards"))
        })
        .unwrap_or_else(|| panic!("a kill step names the wave: {cp}"));
    assert_eq!(kill["count"], 2, "{kill}");
}

/// A constant count emits neither the party count nor a guard — the spawn is
/// byte-identical to a fixed `count` of the same number.
#[test]
fn a_constant_count_is_byte_identical() {
    let plain = build_with(|_| {});
    assert!(!spawn(&plain).contains("#party_n"));
    let zero = build_with(|c| {
        c.quests.content.waves[0].mobs[0].count = MobCount::Scaled(ScaledCount {
            base: 2,
            per_player: 0,
        })
    });
    assert_eq!(spawn(&plain), spawn(&zero));
}

/// `DW0983` judges a name at the stack's maximum: `count: 1` named is a
/// person; `{base: 1, per_player: 1}` named is a crowd at two players.
#[test]
fn a_name_on_a_stack_that_grows_is_dw0983() {
    let dir = common::compiler_fixtures_dir().join("souls-bonfire");
    let loaded = load_campaign_dir(&dir).unwrap();
    let mut c = parse_campaign(&loaded.raw).expect("fixture parses");
    c.quests.content.waves[0].mobs[0].name = Some("Captain".to_string());
    c.quests.content.waves[0].mobs[0].count = 1.into();
    let (d, _) = delvec::compiler::telling::check_name_tags(&c);
    assert!(d.is_empty(), "{d:#?}");
    c.quests.content.waves[0].mobs[0].count = MobCount::Scaled(ScaledCount {
        base: 1,
        per_player: 1,
    });
    let (d, _) = delvec::compiler::telling::check_name_tags(&c);
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].code, "DW0983");
    assert!(d[0].message.contains("× 4"), "{}", d[0].message);
}

/// **A PackTest seats a scaling wave for its own party of one** (spec-0134
/// §4). The batch runs every template on one shared server, so every other
/// template's dummy is a player present at the spawn: a template that ran the
/// wave's own spawn measured them all, seated more bodies than one player
/// meets and failed its count. The spawn measures and then calls
/// `spawn_<wave>_party`; every template states a party of one and calls that.
#[test]
fn a_packtest_seats_a_scaling_wave_for_a_party_of_one() {
    let out = build_with(|c| c.quests.content.waves[0].mobs[0].count = two_plus_one());
    let own = text(
        &out,
        "datapack/data/souls-bonfire/function/spawn_guards.mcfunction",
    );
    assert_eq!(
        own.lines().collect::<Vec<_>>(),
        vec![
            "execute store result score #pn_guards dw.sys if entity @a[tag=!dw_cutscene]",
            "function souls-bonfire:spawn_guards_party",
        ]
    );
    let mut seats = 0;
    for (path, bytes) in out.iter() {
        if !path.starts_with("packtest-datapack/") || !path.ends_with(".mcfunction") {
            continue;
        }
        let t = String::from_utf8_lossy(bytes);
        let lines: Vec<&str> = t.lines().collect();
        assert!(
            !lines.contains(&"function souls-bonfire:spawn_guards"),
            "{path} seats the wave for every player on the batch server:\n{t}"
        );
        for (i, l) in lines.iter().enumerate() {
            if *l == "function souls-bonfire:spawn_guards_party" {
                seats += 1;
                assert_eq!(
                    lines[i - 1],
                    "scoreboard players set #pn_guards dw.sys 1",
                    "{path} states its party before it seats:\n{t}"
                );
            }
        }
    }
    assert!(seats > 0, "some template seats the wave");
}

/// **A re-seat seats the wave the party met** (spec-0134 §4): it calls the
/// seating at the size the wave's own spawn measured, never a fresh count —
/// so a rest re-seats the same composition, and a PackTest that drives a rest
/// after seating for a party of one meets one player's count.
#[test]
fn a_reseat_seats_the_wave_at_the_size_it_was_first_seated_for() {
    let out = build_with(|c| {
        c.quests.content.waves[0].mobs[0].count = two_plus_one();
        c.quests.content.waves[0].respawns_on_rest = true;
    });
    let reseat = text(
        &out,
        "datapack/data/souls-bonfire/function/wave_reseat_guards.mcfunction",
    );
    assert!(
        reseat
            .lines()
            .any(|l| l == "function souls-bonfire:spawn_guards_party"),
        "{reseat}"
    );
    assert!(
        !reseat
            .lines()
            .any(|l| l == "function souls-bonfire:spawn_guards"),
        "{reseat}"
    );
}

/// **A kill-reward template counts one player's bodies** (spec-0134 §4): it
/// seats the wave for its own party of one, so the bodies it expects standing
/// after a rest and the payments it expects over two seatings are the
/// one-player count, never the four-player maximum.
#[test]
fn a_kill_reward_template_counts_the_bodies_its_party_of_one_meets() {
    let mut base = 0;
    let mut max = 0;
    let out = build_with(|c| {
        let w = &mut c.quests.content.waves[0];
        w.mobs[0].count = two_plus_one();
        w.respawns_on_rest = true;
        w.on_kill = Some(
            serde_json::from_str(
                r#"{ "fires": "every-kill",
                     "effects": [ { "type": "play-sound", "sound": "minecraft:entity.experience_orb.pickup" } ] }"#,
            )
            .unwrap(),
        );
        base = w.mobs.iter().map(|m| m.count.base()).sum::<u32>();
        max = w.mobs.iter().map(|m| m.count.max()).sum::<u32>();
    });
    assert!(max > base, "the perturbation scales the wave");
    let t = text(
        &out,
        "packtest-datapack/data/souls-bonfire/test/kill_pays_across_rest_w_guards.mcfunction",
    );
    assert!(
        t.lines()
            .any(|l| l == format!("assert score #kpa_w_guards dw.sys matches {base}")),
        "{t}"
    );
    assert!(
        t.lines()
            .any(|l| l == format!("assert score #kf_w_guards dw.sys matches {}", 2 * base)),
        "{t}"
    );
}

/// **A `first-kill` bundle on a wave that scales pays once per body the party
/// met** (spec-0134 §4 × spec-0074 §6): every seating raises `#wb_<wave>` to
/// the bodies it seated, and the bundle is called while the ledger is below
/// it — never below the four-player maximum, which a party of one, meeting
/// `base` bodies, would be paid up to over the wave's re-seats.
#[test]
fn a_first_kill_bundle_on_a_wave_that_scales_pays_once_per_body_met() {
    let mut max = 0;
    let out = build_with(|c| {
        let w = &mut c.quests.content.waves[0];
        w.mobs[0].count = two_plus_one();
        w.respawns_on_rest = true;
        w.on_kill = Some(
            serde_json::from_str(
                r#"{ "fires": "first-kill",
                     "effects": [ { "type": "play-sound", "sound": "minecraft:entity.experience_orb.pickup" } ] }"#,
            )
            .unwrap(),
        );
        max = w.mobs.iter().map(|m| m.count.max()).sum::<u32>();
    });
    let seat = text(
        &out,
        "datapack/data/souls-bonfire/function/spawn_guards_party.mcfunction",
    );
    assert!(
        seat.lines()
            .any(|l| l == "scoreboard players operation #wb_guards dw.sys > #guards dw.wave"),
        "{seat}"
    );
    let call = "run function souls-bonfire:on_kill_w_guards";
    let calls: Vec<&str> = out
        .iter()
        .filter(|(p, _)| p.starts_with("datapack/") && p.ends_with(".mcfunction"))
        .flat_map(|(_, b)| std::str::from_utf8(b).unwrap().lines())
        .filter(|l| l.ends_with(call))
        .collect();
    assert!(!calls.is_empty(), "the bundle is called from no line");
    for l in &calls {
        assert!(
            l.contains("if score #kf_w_guards dw.sys < #wb_guards dw.sys"),
            "the bundle is capped at something other than the bodies met: {l}"
        );
        assert!(
            !l.contains(&format!("matches ..{}", max - 1)),
            "the bundle is capped at the four-player maximum: {l}"
        );
    }
}
