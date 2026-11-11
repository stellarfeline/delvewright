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

fn spawn(out: &BuildOutput) -> String {
    text(
        out,
        "datapack/data/souls-bonfire/function/spawn_guards.mcfunction",
    )
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
            *l == "execute store result score #party_n dw.sys if entity @a[tag=!dw_cutscene]"
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
                "execute if score #party_n dw.sys matches {need}.. run summon "
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
            l.starts_with("execute if score #party_n dw.sys matches ")
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
