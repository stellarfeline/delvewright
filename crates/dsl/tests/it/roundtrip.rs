//! Canonical round-trip: parse each valid fixture and re-serialize; the output
//! must be byte-identical to the file on disk.
//!
//! Run with `DW_BLESS=1` to rewrite the fixtures into canonical form.

mod common;

use std::fs;

use delvewright_dsl::envelope::Envelope;
use delvewright_dsl::to_canonical_string;
use delvewright_dsl::{
    ClassesContent, DialogueContent, NpcsContent, QuestPlanContent, QuestsContent, WorldContent,
};
use serde::{Serialize, de::DeserializeOwned};

fn roundtrip<T: DeserializeOwned + Serialize>(name: &str) {
    let path = common::valid_dir().join(name);
    let original = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {name}: {e}"));
    let env: Envelope<T> =
        serde_json::from_str(&original).unwrap_or_else(|e| panic!("parse {name}: {e}"));
    let canonical = to_canonical_string(&env).expect("canonical serialize");

    if std::env::var_os("DW_BLESS").is_some() {
        fs::write(&path, &canonical).unwrap();
        return;
    }
    assert_eq!(
        original, canonical,
        "fixture {name} is not in canonical form (run with DW_BLESS=1 to fix)"
    );
}

#[test]
fn roundtrip_all_valid_fixtures() {
    roundtrip::<WorldContent>("world.json");
    roundtrip::<NpcsContent>("npcs.json");
    roundtrip::<ClassesContent>("classes.json");
    roundtrip::<QuestPlanContent>("quest-plan.json");
    roundtrip::<QuestsContent>("quests.json");
    roundtrip::<DialogueContent>("dialogue.json");
}

/// spec-0104 criterion 1: a document declaring none of the guidance fields
/// canonicalises byte for byte with no `guidance` block; one declaring the
/// compass, a highlight and a manual keeps them; `highlight.within` outside
/// 1..=64 and an empty `manual.pages` are refused where they are entered.
#[test]
fn the_guidance_surface_round_trips_and_refuses_its_bounds() {
    let path = common::valid_dir().join("quests.json");
    let original = fs::read_to_string(&path).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&original).unwrap();
    doc["content"].as_object_mut().unwrap().remove("guidance");
    let bare: Envelope<QuestsContent> = serde_json::from_value(doc.clone()).unwrap();
    let bare_text = to_canonical_string(&bare).unwrap();
    assert!(!bare_text.contains("guidance"), "{bare_text}");
    assert_eq!(bare_text, to_canonical_string(&doc).unwrap());

    doc["content"]["guidance"] = serde_json::json!({
        "compass": "hidden",
        "manual": { "title": "Notes", "pages": ["One [[key|use]]."] }
    });
    doc["content"]["quests"][0]["objectives"][0]["compass"] = serde_json::json!("shown");
    doc["content"]["quests"][0]["objectives"][0]["highlight"] = serde_json::json!({ "within": 6 });
    let env: Envelope<QuestsContent> = serde_json::from_value(doc.clone()).unwrap();
    let out = to_canonical_string(&env).unwrap();
    let back: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(back["content"]["guidance"], doc["content"]["guidance"]);
    assert_eq!(
        back["content"]["quests"][0]["objectives"][0]["highlight"],
        serde_json::json!({ "within": 6 })
    );
    let again: Envelope<QuestsContent> = serde_json::from_str(&out).unwrap();
    assert_eq!(to_canonical_string(&again).unwrap(), out);

    for bad in [0, 65] {
        let mut d = doc.clone();
        d["content"]["quests"][0]["objectives"][0]["highlight"] =
            serde_json::json!({ "within": bad });
        let e = serde_json::from_value::<Envelope<QuestsContent>>(d).unwrap_err();
        assert!(e.to_string().contains("1 to 64"), "{e}");
    }
    let mut d = doc.clone();
    d["content"]["guidance"]["manual"]["pages"] = serde_json::json!([]);
    let e = serde_json::from_value::<Envelope<QuestsContent>>(d).unwrap_err();
    assert!(e.to_string().contains("at least one page"), "{e}");
}
