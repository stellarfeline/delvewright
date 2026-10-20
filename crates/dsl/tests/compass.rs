//! `DW1049`: a direction in player-facing text names what the player sees,
//! never a compass point (`dsl::compass`, spec-0116).
//!
//! The rule judges every line of the l10n inventory but a declared name, so the
//! tests plant a compass word in EVERY inventoried string of a fixture and
//! require the refused set to be exactly the inventory less its names — the
//! walk is the inventory's, never a second list of fields.

use std::collections::BTreeSet;

use delvewright_dsl::compass::{
    COMPASS_DIRECTION, CompassBinding, NAME_KINDS, validate_compass_directions,
};
use delvewright_dsl::l10n::{each_string, inventory, key_kind};
use delvewright_dsl::{Campaign, DSL_VERSION};
use serde_json::Value;

fn keep_trial(edit: impl FnOnce(&mut Value)) -> Campaign {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/valid/keep-trial");
    let read =
        |f: &str| std::fs::read_to_string(dir.join(f)).unwrap_or_else(|e| panic!("read {f}: {e}"));
    let mut quests: Value = serde_json::from_str(&read("quests.json")).expect("quests parse");
    quests["dsl_version"] = Value::String(DSL_VERSION.to_string());
    edit(&mut quests);
    delvewright_dsl::parse_campaign(&delvewright_dsl::RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: read("classes.json"),
        quest_plan: read("quest-plan.json"),
        quests: serde_json::to_string(&quests).expect("quests re-serialize"),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("the keep-trial parses")
}

fn is_name(key: &str) -> bool {
    key_kind(key).is_some_and(|k| NAME_KINDS.contains(&k))
}

#[test]
fn the_pristine_fixture_gives_no_compass_direction() {
    let c = keep_trial(|_| {});
    let d = validate_compass_directions(&c);
    assert!(d.is_empty(), "{d:#?}");
    let b = CompassBinding::of(&c);
    assert!(b.judged > 0, "binding: nothing judged");
    println!("{}", b.line());
}

/// A compass word planted in every inventoried string is refused in exactly
/// the lines that are not declared names, once each, naming the key and the
/// word.
#[test]
fn every_judged_line_is_refused_and_no_name_is() {
    let mut c = keep_trial(|q| {
        q["content"]["quests"][0]["on_complete"] =
            serde_json::json!([{ "type": "narrate", "text": "The hall falls quiet." }]);
    });
    // A name ends in a word of its own, so no line can quote the whole name and
    // be exempted by it: the test is about the walk, not the name rule.
    each_string(&mut c, &mut |k, v| {
        if is_name(k) {
            v.push_str(" Go west Zq");
        } else {
            v.push_str(" Go west.");
        }
    });
    let inv = inventory(&c);
    let want: BTreeSet<String> = inv.keys().filter(|k| !is_name(k)).cloned().collect();
    let names = inv.keys().filter(|k| is_name(k)).count();
    assert!(!want.is_empty() && names > 0, "binding: {want:?} / {names}");
    let d = validate_compass_directions(&c);
    let got: BTreeSet<String> = d
        .iter()
        .map(|x| {
            assert_eq!(x.code, "DW1049");
            assert_eq!(x.code, COMPASS_DIRECTION.id());
            assert!(x.message.contains("`west`"), "{}", x.message);
            x.path.trim_start_matches("#/").to_string()
        })
        .collect();
    assert_eq!(got, want);
    assert_eq!(d.len(), want.len(), "one diagnostic per line");
    let key = want.iter().next().unwrap();
    assert!(d[0].message.contains(key.as_str()) || d.iter().any(|x| x.message.contains(key)));
    println!(
        "DW1049 binding: {} line(s) refused of {} inventoried; {} declared name(s) never judged",
        got.len(),
        inv.len(),
        names
    );
}

/// The proper-name rule, both sides: an area named "The North Gate" makes "the
/// north gate" in a line a name; the same word outside the name is refused; a
/// word that only contains a cardinal is not a compass word.
#[test]
fn a_declared_name_is_not_a_direction() {
    let line = |text: &str| {
        let mut c = keep_trial(|q| {
            q["content"]["quests"][0]["on_complete"] =
                serde_json::json!([{ "type": "narrate", "text": text }]);
        });
        c.world.content.areas[0].name = "The North Gate".to_string();
        validate_compass_directions(&c)
    };
    assert!(line("Wait for him by the North Gate.").is_empty());
    assert!(line("wait for him by the north gate").is_empty());
    assert!(line("The road to Westmarch runs past the Northmen's huts.").is_empty());
    let d = line("The North Gate is shut; take the north stair.");
    assert_eq!(d.len(), 1, "{d:#?}");
    assert_eq!(d[0].code, "DW1049");
    assert!(d[0].message.contains("`north`"));
    let d = line("Go NE from the North Gate.");
    assert_eq!(d.len(), 1, "{d:#?}");
    assert!(d[0].message.contains("`NE`"));
    // Without the declaration the same line is refused.
    let mut c = keep_trial(|q| {
        q["content"]["quests"][0]["on_complete"] =
            serde_json::json!([{ "type": "narrate", "text": "Wait by the North Gate." }]);
    });
    assert_eq!(validate_compass_directions(&c).len(), 1);
    // A name is itself never judged.
    c.world.content.areas[0].name = "The West Bay".to_string();
    let d = validate_compass_directions(&c);
    assert!(d.iter().all(|x| !x.path.starts_with("#/area.")), "{d:#?}");
}
