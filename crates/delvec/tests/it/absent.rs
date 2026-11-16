//! The party is every member, present or not (spec-0141).

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use delvec::compiler::absent;
use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn build_dir(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
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
    let mut skins: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for npc in &campaign.npcs.content.npcs {
        if let Some(skin) = &npc.skin {
            let png = std::fs::read(dir.join("skins").join(format!("{}.png", skin.texture_id)))
                .expect("skin png present");
            skins.insert(skin.texture_id.clone(), png);
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &tree,
        &prefabs,
        None,
        &skins,
    )
    .expect("emission succeeds")
}


fn families() -> Vec<(&'static str, BuildOutput)> {
    vec![
        ("hello-world", build_dir(&common::hello_world_dir())),
        ("keep-crawl", build_dir(&common::keep_crawl_dir())),
        ("keep-trial", build_dir(&common::keep_trial_dir())),
        ("keep-vertical", build_dir(&common::keep_vertical_dir())),
        (
            "v04-showcase",
            build_dir(&common::compiler_fixtures_dir().join("v04-showcase")),
        ),
        (
            "v06-checkpoints",
            build_dir(&common::compiler_fixtures_dir().join("v06-checkpoints")),
        ),
        (
            "and-join",
            build_dir(&common::compiler_fixtures_dir().join("and-join")),
        ),
    ]
}

fn text(out: &BuildOutput, path: &str) -> String {
    String::from_utf8(
        out.get(path)
            .unwrap_or_else(|| panic!("the build ships `{path}`"))
            .clone(),
    )
    .unwrap()
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const SHOWCASE: &str = "datapack/data/v04-showcase/function";

// ---------------------------------------------------------------------------
// §3.1 a collect counts the party
// ---------------------------------------------------------------------------

/// The held check sums every player's count and completes on the sum, as a
/// player who carries one: `count` gathered between two inventories completes
/// the objective, which one player's own count never reached.
#[test]
fn a_collect_completes_on_what_the_party_holds() {
    let dir = tmp("absent-collect");
    common::copy_dir_all(&common::compiler_fixtures_dir().join("v04-showcase"), &dir);
    let mut id = String::new();
    common::patch_file(&dir.join("quests.json"), |v| {
        let o = v["content"]["quests"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .flat_map(|q| q["objectives"].as_array_mut().unwrap().iter_mut())
            .find(|o| o["type"] == "collect")
            .expect("the showcase declares a collect");
        o["count"] = serde_json::json!(3);
        id = o["id"].as_str().unwrap().trim_start_matches("obj/").replace('-', "_");
    });
    let out = build_dir(&dir);
    let tick = text(&out, &format!("{SHOWCASE}/tick.mcfunction"));
    let lines: Vec<&str> = tick.lines().collect();
    let store = lines
        .iter()
        .position(|l| l.contains("store result score @s dw.hold if items entity @s container.*"))
        .expect("each player's held count is stored");
    assert_eq!(
        lines[store - 1], "scoreboard players set #hold dw.sys 0",
        "the party's count starts from nothing each tick"
    );
    assert!(
        lines[store + 1].starts_with("execute as @a ")
            && lines[store + 1]
                .ends_with("run scoreboard players operation #hold dw.sys += @s dw.hold"),
        "each player's count is added to the party's: {}",
        lines[store + 1]
    );
    assert!(
        lines[store + 2].ends_with(&format!(
            "if score @s dw.hold matches 1.. if score #hold dw.sys matches 3.. run function \
             v04-showcase:complete_o_{id}"
        )),
        "the objective completes on the party's count, as a player who carries one: {}",
        lines[store + 2]
    );
    // No line completes it on one player's own count.
    assert!(
        !tick.contains("if score @s dw.hold matches 3.."),
        "one inventory's count is not the test:\n{tick}"
    );
    // The two-member template: the count is split, one real tick, done.
    let pt = text(
        &out,
        "packtest-datapack/data/v04-showcase/test/party_collect.mcfunction",
    );
    assert!(pt.contains("dummy dwpcolp1 spawn"), "{pt}");
    let gives: Vec<&str> = pt.lines().filter(|l| l.starts_with("give ")).collect();
    assert_eq!(gives.len(), 2, "{pt}");
    assert!(gives[0].starts_with("give @a[tag=dw_t_pcol,limit=1] ") && gives[0].ends_with(" 2"));
    assert!(gives[1].starts_with("give @a[name=dwpcolp1,limit=1] ") && gives[1].ends_with(" 1"));
    assert!(
        pt.contains(&format!("assert score #party dw.o_{id} matches 1")),
        "{pt}"
    );
    let b = emit_absent_binding_line(&out, "v04-showcase");
    assert!(b.contains("party_collect"), "{b}");
}

/// The template the suite carries, named in its own header: what the binding
/// line counts.
fn emit_absent_binding_line(out: &BuildOutput, ns: &str) -> String {
    out.keys()
        .filter(|k| k.starts_with(&format!("packtest-datapack/data/{ns}/test/party_")))
        .cloned()
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// §3.2 a stack for the party is a grant
// ---------------------------------------------------------------------------

/// A `carrier: all` give under a completion bundle is a call to its grant; the
/// grant gives every present player and counts the issue on them and on the
/// party; the tick hands a member who is behind the next stack.
#[test]
fn a_party_give_is_a_grant_owed_to_a_member_who_was_away() {
    let out = build_dir(&common::compiler_fixtures_dir().join("v04-showcase"));
    let talk = text(&out, &format!("{SHOWCASE}/complete_o_talk.mcfunction"));
    assert!(
        talk.lines().any(|l| l == "function v04-showcase:grant_0"),
        "{talk}"
    );
    assert!(!talk.contains("give @a"), "no bare party give is left: {talk}");
    let grant = text(&out, &format!("{SHOWCASE}/grant_0.mcfunction"));
    let g: Vec<&str> = grant.lines().collect();
    assert!(g[0].starts_with("give @a minecraft:paper["), "{grant}");
    assert_eq!(
        &g[1..],
        &[
            "scoreboard players add @a dw.gr.0 1",
            "scoreboard players add @a dw.gr 1",
            "scoreboard players add #all dw.gr.0 1",
            "scoreboard players add #all dw.gr 1",
        ],
        "{grant}"
    );
    let stack = g[0].strip_prefix("give @a ").unwrap();
    assert_eq!(
        text(&out, &format!("{SHOWCASE}/grant_owed_0.mcfunction")),
        format!(
            "give @s {stack}\nscoreboard players add @s dw.gr.0 1\nscoreboard players add @s \
             dw.gr 1\n"
        )
    );
    assert_eq!(
        text(&out, &format!("{SHOWCASE}/grant_owed.mcfunction")),
        "scoreboard players add @s dw.gr.0 0\nexecute if score @s dw.gr.0 < #all dw.gr.0 run \
         function v04-showcase:grant_owed_0\n"
    );
    let tick = text(&out, &format!("{SHOWCASE}/tick.mcfunction"));
    let lines: Vec<&str> = tick.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains("grant_owed"))
        .expect("the tick carries the owed line");
    assert_eq!(
        lines.iter().filter(|l| l.contains("grant_owed")).count(),
        1,
        "one owed line"
    );
    let who = lines[at]
        .strip_prefix("execute as ")
        .and_then(|l| l.split(' ').next())
        .unwrap();
    assert_eq!(
        lines[at],
        format!(
            "execute as {who} unless score @s dw.gr = #all dw.gr run function \
             v04-showcase:grant_owed"
        )
    );
    assert_eq!(
        lines[at - 1],
        format!("scoreboard players add {who} dw.gr 0"),
        "the total's entry is created before it is compared"
    );
    let setup = text(&out, &format!("{SHOWCASE}/setup.mcfunction"));
    for l in [
        "scoreboard objectives add dw.gr dummy",
        "scoreboard players add #all dw.gr 0",
        "scoreboard objectives add dw.gr.0 dummy",
        "scoreboard players add #all dw.gr.0 0",
    ] {
        assert!(setup.lines().any(|x| x == l), "setup carries `{l}`");
    }
    // The late-member template: issued, a member spawned after it, handed once.
    let pt = text(
        &out,
        "packtest-datapack/data/v04-showcase/test/party_grant_0.mcfunction",
    );
    let at = |needle: &str| {
        pt.lines()
            .position(|l| l.contains(needle))
            .unwrap_or_else(|| panic!("`{needle}` in:\n{pt}"))
    };
    assert!(at("function v04-showcase:grant_0") < at("dummy dwpg0p1 spawn"), "{pt}");
    assert!(at("dummy dwpg0p1 spawn") < at("run function v04-showcase:grant_owed"), "{pt}");
    assert!(pt.contains("assert score #pg_0_a dw.sys matches 1"), "{pt}");
    assert!(pt.contains("assert score #pg_0_b dw.sys matches 1"), "{pt}");
}

/// A give that names who is given is not a grant: with every give the one
/// carrier's, the campaign emits no grant at all.
#[test]
fn a_give_that_names_its_player_stays_with_that_player() {
    let dir = tmp("absent-solo");
    common::copy_dir_all(&common::compiler_fixtures_dir().join("v04-showcase"), &dir);
    common::patch_file(&dir.join("quests.json"), |v| {
        let mut n = 0;
        fn walk(v: &mut serde_json::Value, n: &mut usize) {
            match v {
                serde_json::Value::Object(m) => {
                    if m.get("type").and_then(|t| t.as_str()) == Some("give-item") {
                        m.insert("carrier".into(), serde_json::json!("one"));
                        *n += 1;
                    }
                    for x in m.values_mut() {
                        walk(x, n);
                    }
                }
                serde_json::Value::Array(a) => a.iter_mut().for_each(|x| walk(x, n)),
                _ => {}
            }
        }
        walk(v, &mut n);
        assert!(n > 0, "the showcase gives an item");
    });
    let out = build_dir(&dir);
    let talk = text(&out, &format!("{SHOWCASE}/complete_o_talk.mcfunction"));
    assert!(
        talk.lines().any(|l| l.starts_with("give @s minecraft:paper[")),
        "{talk}"
    );
    assert!(
        !out.keys().any(|k| k.contains("/function/grant_")),
        "a campaign with no party give emits no grant"
    );
    let tick = text(&out, &format!("{SHOWCASE}/tick.mcfunction"));
    assert!(!tick.contains("dw.gr"), "and no owed line");
    assert!(!text(&out, &format!("{SHOWCASE}/setup.mcfunction")).contains("dw.gr"));
}

// ---------------------------------------------------------------------------
// §3.3 a class taken late arrives where the party is
// ---------------------------------------------------------------------------

/// With a checkpoint declared, the class apply runs the respawn's own seat
/// dispatch after its entry teleport; with none, it does not.
#[test]
fn a_class_taken_once_a_checkpoint_is_armed_arrives_on_it() {
    let out = build_dir(&common::compiler_fixtures_dir().join("v06-checkpoints"));
    let apply = out
        .iter()
        .find(|(p, _)| p.contains("/function/class_apply_"))
        .map(|(_, b)| String::from_utf8_lossy(b).into_owned())
        .expect("the fixture has a class");
    let lines: Vec<&str> = apply.lines().collect();
    let entry = lines
        .iter()
        .position(|l| l.starts_with("teleport @s "))
        .expect("the entry teleport");
    assert!(
        lines[entry + 1].starts_with("execute if score #cp dw.sys matches 0 run function ")
            && lines[entry + 1].ends_with(":cp_seat_0"),
        "the seat dispatch follows the entry teleport: {apply}"
    );
    let seats = out.keys().filter(|k| k.contains("/function/cp_seat_")).count();
    assert_eq!(
        lines.iter().filter(|l| l.contains(":cp_seat_")).count(),
        seats,
        "one dispatch line per checkpoint seat"
    );
    // A campaign that declares no checkpoint has no dispatch.
    let plain = build_dir(&common::hello_world_dir());
    for (p, b) in plain.iter().filter(|(p, _)| p.contains("/function/class_apply_")) {
        assert!(!String::from_utf8_lossy(b).contains("#cp"), "{p}");
    }
}

// ---------------------------------------------------------------------------
// §4 the census
// ---------------------------------------------------------------------------

/// Over every fixture family the census binds, and names every site it finds.
#[test]
fn every_family_s_writes_are_served_and_its_picks_named() {
    let mut writes = 0;
    let mut picks = 0;
    for (name, out) in families() {
        let c = absent::census(&out);
        assert!(c.functions > 0, "{name}");
        assert!(c.unserved.is_empty(), "{name}: {:?}", c.unserved);
        assert!(c.unnamed.is_empty(), "{name}: {:?}", c.unnamed);
        let ledger: serde_json::Value =
            serde_json::from_slice(out.get("validation/absent-census.json").unwrap()).unwrap();
        assert_eq!(ledger["lasting_writes_onto_every_present_player"], c.writes, "{name}");
        assert_eq!(ledger["picks_of_one_player"], c.picks, "{name}");
        assert_eq!(ledger["unserved"], 0);
        assert_eq!(ledger["unnamed"], 0);
        writes += c.writes;
        picks += c.picks;
    }
    assert!(writes > 0 && picks > 0, "the families bind both rules: {writes} / {picks}");
}

/// `DW1126`: a lasting write onto the players present, planted in a completion
/// bundle, is refused by name — the perturbation only this gate could catch.
#[test]
fn a_planted_write_onto_the_present_is_dw1126() {
    let mut out = build_dir(&common::compiler_fixtures_dir().join("v04-showcase"));
    let path = format!("{SHOWCASE}/complete_o_talk.mcfunction");
    let body = text(&out, &path);
    out.insert(path, format!("{body}tag @a add dw_was_there\n").into_bytes());
    let c = absent::census(&out);
    assert_eq!(c.unserved.len(), 1, "{:?}", c.unserved);
    assert_eq!(c.unserved[0].1, "tag @a add dw_was_there");
    let refusal = absent::check(&out).expect_err("an unserved write is refused");
    assert_eq!(refusal.code.to_string(), "DW1126");
    assert!(refusal.message.contains("ENGINE SELF-CHECK"), "{}", refusal.message);
    assert!(refusal.message.contains("complete_o_talk"), "{}", refusal.message);
}

/// `DW1127`: a pick of one player planted in a completion bundle is refused by
/// name; the same `@p` inside an authored line is text.
#[test]
fn a_planted_pick_of_one_player_is_dw1127() {
    let built = build_dir(&common::compiler_fixtures_dir().join("v04-showcase"));
    let path = format!("{SHOWCASE}/complete_o_talk.mcfunction");
    let body = text(&built, &path);
    let mut out = built.clone();
    out.insert(
        path.clone(),
        format!("{body}execute at @p run summon minecraft:marker ~ ~ ~\n").into_bytes(),
    );
    let c = absent::census(&out);
    assert_eq!(c.unnamed.len(), 1, "{:?}", c.unnamed);
    let refusal = absent::check(&out).expect_err("an unnamed pick is refused");
    assert_eq!(refusal.code.to_string(), "DW1127");
    assert!(refusal.message.contains("ENGINE SELF-CHECK"), "{}", refusal.message);

    let mut told = built;
    told.insert(
        path,
        format!("{body}tellraw @a {{\"text\":\"ask @p or @r , whoever is near\"}}\n").into_bytes(),
    );
    assert!(absent::check(&told).is_ok(), "an authored line is not a selector");
}

// ---------------------------------------------------------------------------
// the gallery
// ---------------------------------------------------------------------------

/// The engine's own campaign: every write served, every pick named, and the
/// three absent-member templates in its suite.
#[test]
fn the_gallery_binds_the_census_and_carries_the_party_templates() {
    let dir = tmp("absent-gallery");
    let (campaign, prefabs) = common::gallery_primary_at(&dir.join("gallery"));
    let built = dir.join("build");
    let r = Command::new(BIN)
        .arg("build")
        .arg(&campaign)
        .arg("-o")
        .arg(&built)
        .arg("--prefabs")
        .arg(&prefabs)
        .output()
        .unwrap();
    let log = String::from_utf8_lossy(&r.stderr);
    assert!(r.status.success(), "{log}");
    let census = log
        .lines()
        .find(|l| l.starts_with("absent-member census (spec-0141): "))
        .unwrap_or_else(|| panic!("the build prints the census line:\n{log}"));
    assert!(census.contains(" 0 unserved (DW1126)") && census.contains(" 0 unnamed (DW1127)"));
    let ledger: serde_json::Value = serde_json::from_slice(
        &std::fs::read(built.join("validation/absent-census.json")).unwrap(),
    )
    .unwrap();
    assert!(ledger["lasting_writes_onto_every_present_player"].as_u64().unwrap() > 0);
    assert!(ledger["picks_of_one_player"].as_u64().unwrap() > 0);
    let binding = log
        .lines()
        .find(|l| l.starts_with("absent-member binding (spec-0141): "))
        .unwrap_or_else(|| panic!("the build prints the binding line:\n{log}"));
    let tests = built.join("packtest-datapack/data/gallery/test");
    assert!(tests.join("party_collect.mcfunction").is_file(), "{binding}");
    assert!(tests.join("party_class_arrives.mcfunction").is_file(), "{binding}");
    let grants = std::fs::read_dir(&tests)
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with("party_grant_")
        })
        .count();
    assert!(grants > 0, "{binding}");
    assert!(
        binding.contains(&format!("{grants} grant(s), {grants} judged")),
        "{binding}"
    );
    eprintln!("{census}\n{binding}");
}
