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

/// spec-0104 §5.3: a document declaring no `teaching` canonicalises byte for
/// byte as it did — no `guidance` block appears; one that declares it keeps a
/// `null` as a declaration and a line as a line, and drops an undeclared class.
#[test]
fn a_teaching_block_keeps_its_nulls_and_drops_what_it_does_not_declare() {
    let path = common::valid_dir().join("quests.json");
    let original = fs::read_to_string(&path).unwrap();
    let mut doc: serde_json::Value = serde_json::from_str(&original).unwrap();
    // The fixture without its `guidance` block: canonical in, canonical out.
    doc["content"].as_object_mut().unwrap().remove("guidance");
    let bare: Envelope<QuestsContent> = serde_json::from_value(doc.clone()).unwrap();
    let bare_text = to_canonical_string(&bare).unwrap();
    assert!(!bare_text.contains("guidance"), "{bare_text}");
    assert_eq!(bare_text, to_canonical_string(&doc).unwrap());
    doc["content"]["guidance"] = serde_json::json!({
        "teaching": { "use-block": "Things answer to [[key|use]].", "attack-body": null }
    });
    let env: Envelope<QuestsContent> = serde_json::from_value(doc).unwrap();
    let t = &env.content.guidance.teaching;
    assert_eq!(
        t.use_block,
        Some(delvewright_dsl::TeachLine::Line(
            "Things answer to [[key|use]].".into()
        ))
    );
    assert_eq!(t.attack_body, Some(delvewright_dsl::TeachLine::Level));
    assert_eq!(t.use_body, None);
    let out = to_canonical_string(&env).unwrap();
    let back: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        back["content"]["guidance"]["teaching"],
        serde_json::json!({ "use-block": "Things answer to [[key|use]].", "attack-body": null })
    );
    let again: Envelope<QuestsContent> = serde_json::from_str(&out).unwrap();
    assert_eq!(to_canonical_string(&again).unwrap(), out);
}
