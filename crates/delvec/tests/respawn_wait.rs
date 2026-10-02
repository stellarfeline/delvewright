//! spec-0077: a fallen player waits — `world.respawn_wait { seconds, alone }`.
//!
//! Driven by the `souls-bonfire` fixture (a bonfire, a wave it re-seats), with
//! the wait declared on the parsed campaign. The live behaviour is proven by
//! `harness/probe/respawn-wait-party.ts`; these pin what the build emits and
//! what it refuses.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::observer;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Campaign, RespawnWait, parse_campaign, validate_campaign_with};

const NS: &str = "souls-bonfire";

fn fixture_dir() -> std::path::PathBuf {
    common::compiler_fixtures_dir().join(NS)
}

fn fixture() -> Campaign {
    let loaded = load_campaign_dir(&fixture_dir()).unwrap();
    parse_campaign(&loaded.raw).expect("souls-bonfire parses")
}

fn with_wait(seconds: u16, alone: bool) -> Campaign {
    let mut c = fixture();
    c.world.content.respawn_wait = Some(RespawnWait { seconds, alone });
    c
}

fn codes(c: &Campaign) -> Vec<String> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        c,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
    .into_iter()
    .map(|d| d.code.to_string())
    .collect()
}

fn try_build(c: &Campaign) -> Result<BuildOutput, BuildFailure> {
    let loaded = load_campaign_dir(&fixture_dir()).unwrap();
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    assert!(codes(c).is_empty(), "{:?}", codes(c));
    let plan = Plan::build(c, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &c.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(
                fixture_dir()
                    .join("skins")
                    .join(format!("{}.png", skin.texture_id)),
            )
            .unwrap();
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &skins,
    )
}

fn build(c: &Campaign) -> BuildOutput {
    try_build(c).unwrap_or_else(|e| panic!("builds: {e:?}"))
}

fn fn_body<'a>(out: &'a BuildOutput, name: &str) -> &'a str {
    let path = format!("datapack/data/{NS}/function/{name}.mcfunction");
    std::str::from_utf8(
        out.get(&path)
            .unwrap_or_else(|| panic!("missing fn {name}")),
    )
    .unwrap()
}

fn datapack_text(out: &BuildOutput) -> String {
    out.iter()
        .filter(|(p, _)| p.starts_with("datapack/"))
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Absent, nothing of the wait is emitted: no `rw_*` function, no clock, no
/// census ledger, and the respawn edge fires `cp_respawn_fire` directly.
#[test]
fn absent_emits_nothing_of_the_wait() {
    let out = build(&fixture());
    assert!(!out.keys().any(|p| p.contains("/function/rw_")));
    assert!(!datapack_text(&out).contains("dw.rwait"));
    assert!(!out.contains_key("validation/observer-census.json"));
    assert!(
        fn_body(&out, "cp_respawn_check").contains(&format!("run function {NS}:cp_respawn_fire"))
    );
}

/// Acceptance 3: the edge puts the player in spectator under the observation
/// tag and starts the clock; the release compares against `seconds × 20`; the
/// wipe detector counts a waiting player as down; `alone: false` gates the wait
/// on a second player present.
#[test]
fn declared_the_edge_waits_and_the_wipe_counts_a_waiter_as_down() {
    let out = build(&with_wait(10, false));
    let check = fn_body(&out, "cp_respawn_check");
    assert!(
        check.contains(&format!("run function {NS}:rw_begin")),
        "{check}"
    );
    assert!(!check.contains("cp_respawn_fire"), "{check}");

    let start = fn_body(&out, "rw_start");
    for line in [
        "scoreboard players set @s dw.rwait 1",
        "tag @s add dw_cutscene",
        "gamemode spectator @s",
    ] {
        assert!(
            start.lines().any(|l| l == line),
            "rw_start lacks `{line}`: {start}"
        );
    }

    let tick = fn_body(&out, "rw_tick");
    assert!(
        tick.contains(&format!(
            "execute if score @s dw.rwait matches 200.. run return run function {NS}:rw_release"
        )),
        "{tick}"
    );
    assert!(tick.contains("delvewright.ui.respawn.wait"), "{tick}");
    assert!(
        tick.contains(&format!(
            "unless predicate {NS}:sneak_held run spectate @p[tag=!dw_cutscene,"
        )),
        "{tick}"
    );
    assert!(out.contains_key(&format!("datapack/data/{NS}/predicate/sneak_held.json")));

    let begin = fn_body(&out, "rw_begin");
    assert!(
        begin.contains("if score #present dw.sys matches 2.. if entity @s[tag=!dw_wiped]"),
        "{begin}"
    );
    assert!(
        !begin.contains("matches ..1"),
        "alone: false never waits alone: {begin}"
    );
    assert!(
        begin.ends_with(&format!("function {NS}:cp_respawn_fire\n")),
        "{begin}"
    );

    let tickfn = fn_body(&out, "tick");
    assert!(
        tickfn.contains(
            "execute as @a unless data entity @s {Health:0.0f} unless score @s dw.rwait matches 1.. run scoreboard players add #alive dw.sys 1"
        ),
        "the wipe detector counts a waiting player as down"
    );
    assert!(tickfn.contains(&format!(
        "execute if score #alive dw.sys matches 0 as @a if score @s dw.rwait matches 1.. run function {NS}:rw_release"
    )));

    let release = fn_body(&out, "rw_release");
    assert!(release.contains("gamemode adventure @s"), "{release}");
    assert!(release.contains("tag @s remove dw_cutscene"), "{release}");
    assert!(
        release.ends_with(&format!("function {NS}:cp_respawn_fire\n")),
        "{release}"
    );
}

/// `alone: true` waits with nobody else present, holds the watcher on the
/// checkpoint, and ends a wait at a wipe only with a second player present.
#[test]
fn alone_true_waits_alone() {
    let out = build(&with_wait(10, true));
    let begin = fn_body(&out, "rw_begin");
    assert!(
        begin.contains(&format!(
            "execute if score #present dw.sys matches ..1 run return run function {NS}:rw_start"
        )),
        "{begin}"
    );
    assert!(fn_body(&out, "rw_tick").contains(&format!("run function {NS}:rw_watch_fire")));
    assert!(fn_body(&out, "rw_watch_fire").contains(&format!("function {NS}:cp_seat_0")));
    assert!(fn_body(&out, "tick").contains(
        "execute if score #alive dw.sys matches 0 if score #present dw.sys matches 2.. as @a"
    ));
}

/// Acceptance 2's rule, on the fixture: each field moves an emitted byte.
#[test]
fn each_field_moves_an_emitted_byte() {
    let ten = build(&with_wait(10, false));
    let eleven = build(&with_wait(11, false));
    let alone = build(&with_wait(10, true));
    assert_ne!(fn_body(&ten, "rw_tick"), fn_body(&eleven, "rw_tick"));
    assert_ne!(fn_body(&ten, "rw_begin"), fn_body(&alone, "rw_begin"));
}

/// Acceptance 4: every positional player selector in the build excludes the
/// observation tag or stands at an allowed site, and both counts bind.
#[test]
fn every_positional_player_selector_is_guarded_or_allowed() {
    let out = build(&with_wait(10, false));
    let c = observer::census(&out);
    let allowed: usize = c.allowed.values().sum();
    println!("{}", c.binding());
    for (site, n) in &c.allowed {
        println!("  allowed `{site}`: {n}");
    }
    assert!(c.selectors > 0 && c.functions > 0, "{c:?}");
    assert!(c.guarded > 0, "nothing excludes the tag: {c:?}");
    assert!(allowed > 0, "no allowed site bound: {c:?}");
    assert_eq!(c.guarded + allowed + c.unguarded.len(), c.selectors);
    assert!(c.unguarded.is_empty(), "{:?}", c.unguarded);
    let ledger = out
        .get("validation/observer-census.json")
        .expect("the census ledger ships with a declared wait");
    let ledger: serde_json::Value = serde_json::from_slice(ledger).unwrap();
    assert_eq!(ledger["unguarded"], 0);
    assert_eq!(ledger["positional_player_selectors"], c.selectors);
}

/// `DW0926`: the census reds when a guard is taken away — the perturbation
/// only this gate could catch — and the build-tier refusal says it is the
/// engine's defect.
#[test]
fn the_census_finds_a_selector_whose_guard_is_removed() {
    let mut out = build(&with_wait(10, false));
    let (path, body) = out
        .iter()
        .find(|(p, b)| {
            p.starts_with("datapack/")
                && p.ends_with(".mcfunction")
                && String::from_utf8_lossy(b).contains(",tag=!dw_cutscene]")
        })
        .map(|(p, b)| (p.clone(), String::from_utf8_lossy(b).into_owned()))
        .expect("some selector carries the guard");
    out.insert(
        path,
        body.replacen(",tag=!dw_cutscene]", "]", 1).into_bytes(),
    );
    let c = observer::census(&out);
    assert_eq!(c.unguarded.len(), 1, "{:?}", c.unguarded);
    let refusal = observer::check(&out).expect_err("an unguarded selector is refused");
    assert_eq!(refusal.code.to_string(), "DW0926");
    assert!(
        refusal.message.contains("ENGINE SELF-CHECK"),
        "{}",
        refusal.message
    );
}

/// `DW0925`: `seconds` outside `1..=120`.
#[test]
fn dw0925_refuses_seconds_out_of_range() {
    for bad in [0u16, 121] {
        assert!(
            codes(&with_wait(bad, false)).iter().any(|c| c == "DW0925"),
            "seconds = {bad}"
        );
    }
    for ok in [1u16, 120] {
        assert!(codes(&with_wait(ok, false)).is_empty(), "seconds = {ok}");
    }
}

/// `DW0925`: a wait in a campaign with no checkpoint or bonfire to come back to.
#[test]
fn dw0925_refuses_a_wait_with_no_checkpoint() {
    let loaded = load_campaign_dir(&fixture_dir()).unwrap();
    let mut raw = loaded.raw.clone();
    let mut quests: serde_json::Value = serde_json::from_str(&raw.quests).unwrap();
    fn strip(v: &mut serde_json::Value) -> usize {
        let mut n = 0;
        match v {
            serde_json::Value::Array(items) => {
                let before = items.len();
                items.retain(|e| e.get("type").and_then(|t| t.as_str()) != Some("bonfire"));
                n += before - items.len();
                for e in items {
                    n += strip(e);
                }
            }
            serde_json::Value::Object(m) => {
                for (_, e) in m.iter_mut() {
                    n += strip(e);
                }
            }
            _ => {}
        }
        n
    }
    assert_eq!(
        strip(&mut quests),
        1,
        "the fixture's one bonfire is removed"
    );
    raw.quests = serde_json::to_string(&quests).unwrap();
    let mut c = parse_campaign(&raw).expect("parses without its bonfire");
    assert!(!delvewright_dsl::validate::declares_checkpoint(&c));
    let without: Vec<String> = codes(&c);
    assert!(!without.iter().any(|c| c == "DW0925"), "{without:?}");
    c.world.content.respawn_wait = Some(RespawnWait {
        seconds: 10,
        alone: false,
    });
    let with: Vec<String> = codes(&c);
    assert_eq!(
        with.iter().filter(|c| *c == "DW0925").count(),
        1,
        "{with:?}"
    );
}
