//! Canonical round-trip: parse each valid fixture and re-serialize; the output
//! must be byte-identical to the file on disk.
//!
//! Run with `DW_BLESS=1` to rewrite the fixtures into canonical form.

mod common;

use std::fs;

use delvewright_dsl::envelope::Envelope;
use delvewright_dsl::stages::{
    ClassesContent, NpcsContent, QuestPlanContent, QuestsContent, WorldContent,
};
use delvewright_dsl::to_canonical_string;
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
}
