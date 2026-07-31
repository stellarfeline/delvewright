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
    let tree = gallery::emit("demo", &candidates(), 4);
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
    let tree2 = gallery::emit("demo", &candidates(), 4);
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
    let tree = gallery::emit("demo", &candidates(), 4);
    let layout_json = String::from_utf8(tree["gallery-layout.json"].clone()).unwrap();
    // the exact orchestrator harvester parser must accept it.
    let layout = Layout::from_json(&layout_json).unwrap();
    assert_eq!(layout.campaign_id, "demo");
    assert!(layout.areas.iter().any(|a| a.id == "acme/keep/gatehouse"));
}

#[test]
fn curation_round_trips_dw_notes_into_a_per_asset_report() {
    let tree = gallery::emit("demo", &candidates(), 4);
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
