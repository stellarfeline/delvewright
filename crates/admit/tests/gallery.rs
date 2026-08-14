//! Gallery world emission + the `dw.note` curation round-trip.

use delvewright_admit::fixtures;
use delvewright_admit::gallery::{self, Candidate};
use delvewright_orchestrator::Layout;

fn candidates() -> Vec<Candidate> {
    let a = fixtures::clean_room().write();
    let b = fixtures::dark_room().write();
    vec![
        Candidate::from_nbt("acme/keep/gatehouse", "gatehouse", a).unwrap(),
        Candidate::from_nbt("acme/crypt/hall", "hall", b).unwrap(),
    ]
}

#[test]
fn gallery_emits_a_bootable_tree() {
    let tree =
        gallery::emit("demo", &candidates(), 4).expect("gallery emits valid 1.21.11 commands");
    for required in [
        "datapack/pack.mcmeta",
        "datapack/data/minecraft/tags/function/load.json",
        "datapack/data/minecraft/tags/function/tick.json",
        "datapack/data/admit/function/place.mcfunction",
        "datapack/data/admit/function/finish.mcfunction",
        "datapack/data/admit/function/stamp.mcfunction",
        "datapack/data/admit/function/emit.mcfunction",
        "datapack/data/admit/structure/acme/keep/gatehouse.nbt",
        "datapack/data/admit/structure/acme/crypt/hall.nbt",
        "gallery-layout.json",
        "server/server.properties",
    ] {
        assert!(tree.contains_key(required), "missing {required}");
    }
    // deterministic: same inputs -> same bytes.
    let tree2 =
        gallery::emit("demo", &candidates(), 4).expect("gallery emits valid 1.21.11 commands");
    assert_eq!(tree, tree2);

    // structure bytes are the verbatim candidate nbt.
    assert_eq!(
        tree["datapack/data/admit/structure/acme/keep/gatehouse.nbt"],
        fixtures::clean_room().write()
    );

    // place function targets each piece.
    let place =
        String::from_utf8(tree["datapack/data/admit/function/place.mcfunction"].clone()).unwrap();
    assert!(place.contains("place template admit:acme/keep/gatehouse"));
    // emit stamps the DelveNote line the harvester parses.
    let emit =
        String::from_utf8(tree["datapack/data/admit/function/emit.mcfunction"].clone()).unwrap();
    assert!(emit.contains("[DelveNote]"));
    // server world is creative for browsing.
    let props = String::from_utf8(tree["server/server.properties"].clone()).unwrap();
    assert!(props.contains("gamemode=creative"));
}

#[test]
fn gallery_layout_is_orchestrator_compatible() {
    let tree =
        gallery::emit("demo", &candidates(), 4).expect("gallery emits valid 1.21.11 commands");
    let layout_json = String::from_utf8(tree["gallery-layout.json"].clone()).unwrap();
    // the exact orchestrator harvester parser must accept it.
    let layout = Layout::from_json(&layout_json).unwrap();
    assert_eq!(layout.campaign_id, "demo");
    assert!(layout.areas.iter().any(|a| a.id == "acme/keep/gatehouse"));
}

#[test]
fn curation_round_trips_dw_notes_into_a_per_asset_report() {
    let tree =
        gallery::emit("demo", &candidates(), 4).expect("gallery emits valid 1.21.11 commands");
    let layout_json = String::from_utf8(tree["gallery-layout.json"].clone()).unwrap();

    // a realistic gallery playtest log: two stamps + notes, one per piece, plus a
    // stamp outside any piece (area=none).
    let log = "\
[12:00:20] [Server thread/INFO]: [creator] [DelveNote] pos=[3,65,3] area=acme/keep/gatehouse quests= nearest_npc=none
[12:00:23] [Server thread/INFO]: <creator> lovely arch, keep it
[12:01:00] [Server thread/INFO]: [creator] [DelveNote] pos=[20,65,3] area=acme/crypt/hall quests= nearest_npc=none
[12:01:03] [Server thread/INFO]: <creator> too dark, reject
[12:02:00] [Server thread/INFO]: [creator] [DelveNote] pos=[-3,65,-3] area=none quests= nearest_npc=none
[12:02:02] [Server thread/INFO]: <creator> standing on the spawn platform
";
    let report = gallery::curate(log, &layout_json).unwrap();
    assert_eq!(report.gallery_id, "demo");
    assert_eq!(
        report.assets["acme/keep/gatehouse"][0].text,
        "lovely arch, keep it"
    );
    assert_eq!(report.assets["acme/crypt/hall"][0].text, "too dark, reject");
    assert_eq!(report.unresolved.len(), 1);

    // report JSON is machine-readable.
    let json = report.to_json();
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["version"], "0.1.0");
}

#[test]
fn merge_into_card_is_idempotent() {
    use delvewright_admit::catalog::CurationNote;
    let notes = vec![CurationNote {
        at: "12:00:20".into(),
        text: "lovely arch".into(),
        pos: [3, 65, 3],
    }];
    let cur = gallery::merge_into_card(&notes, None);
    assert_eq!(cur.notes.len(), 1);
    // merging the same notes again does not duplicate.
    let cur2 = gallery::merge_into_card(&notes, Some(cur));
    assert_eq!(cur2.notes.len(), 1);
}

/// Every `.mcfunction` the gallery writes parses on the pinned server (task #70).
///
/// The gallery emitted four legacy camelCase gamerules and a `text_opacity:255b`
/// for as long as it has existed. Both are refused by 1.21.11, and a refused line
/// costs the WHOLE function: measured on a pinned 1.21.11 server (id 1.21.11,
/// data 4671) booted on the gallery's own datapack, the log carried
///
///   Failed to load function admit:load
///   Failed to load function admit:finish
///   Couldn't load tag minecraft:load as it is missing following references: admit:load
///
/// and the world that came up had no objectives (`There are no objectives`),
/// `advance_time` still `true`, nothing forceloaded (`That position is not
/// loaded`), no piece placed and no label summoned. The tool was inert, and every
/// existing test passed, because no test and no operator ever read what the
/// server said back.
///
/// The gate is stated with its binding count so it cannot pass by matching
/// nothing (CLAUDE.md: a green gate that binds to nothing is vacuous).
#[test]
fn every_emitted_function_parses_on_the_pinned_server() {
    let tree = gallery::emit("demo", &candidates(), 4).expect("gallery emits valid commands");
    let functions = tree.keys().filter(|p| p.ends_with(".mcfunction")).count();
    assert_eq!(
        functions, 6,
        "expected the gallery's six functions to be checked, not {functions}"
    );
    let lines: usize = tree
        .iter()
        .filter(|(p, _)| p.ends_with(".mcfunction"))
        .map(|(_, b)| String::from_utf8_lossy(b).lines().count())
        .sum();
    assert!(lines >= 20, "only {lines} command lines bound the gate");
    assert!(gallery::validate_functions(&tree).is_empty());

    // The named world state, in the emitted bytes: the four rules 1.21.11
    // actually has, and an opacity a signed byte can hold.
    let load =
        String::from_utf8(tree["datapack/data/admit/function/load.mcfunction"].clone()).unwrap();
    for rule in [
        "gamerule advance_time false",
        "gamerule advance_weather false",
        "gamerule spawn_mobs false",
        "gamerule immediate_respawn true",
    ] {
        assert!(load.contains(rule), "missing `{rule}`");
    }
    let finish =
        String::from_utf8(tree["datapack/data/admit/function/finish.mcfunction"].clone()).unwrap();
    assert!(finish.contains("text_opacity:-1b"));
}

/// The gate fails in the direction the code actually drifted (CLAUDE.md's
/// one-directional-falsifiability rule): a gate proven only against lines that
/// were already right proves nothing. These are the EXACT lines this file
/// shipped before task #70, run through the exact check that now guards
/// emission.
#[test]
fn the_command_gate_rejects_the_lines_that_shipped() {
    for bad in [
        "gamerule doDaylightCycle false", // check-live-commands: allow (negative fixture — the exact line that shipped)
        "gamerule doWeatherCycle false", // check-live-commands: allow (negative fixture — the exact line that shipped)
        "gamerule doMobSpawning false", // check-live-commands: allow (negative fixture — the exact line that shipped)
        "gamerule doImmediateRespawn true", // check-live-commands: allow (negative fixture — the exact line that shipped)
        "summon minecraft:text_display 3 70 3 {Tags:[\"admit_label\"],billboard:\"center\",\
         text:'{\"text\":\"gatehouse\"}',text_opacity:255b,see_through:1b}",
    ] {
        let mut tree = std::collections::BTreeMap::new();
        tree.insert(
            "datapack/data/admit/function/load.mcfunction".to_string(),
            format!("{bad}\n").into_bytes(),
        );
        let errors = gallery::validate_functions(&tree);
        assert_eq!(errors.len(), 1, "the gate must reject `{bad}`");
    }
}
