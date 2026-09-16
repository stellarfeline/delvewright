#![allow(dead_code)]
//! Shared fixture-loading helpers for the integration tests.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use delvewright_dsl::{RawCampaign, Stage};
use serde::Deserialize;

pub fn valid_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/valid/hello-world")
}

pub fn invalid_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/invalid")
}

pub fn read_valid(name: &str) -> String {
    fs::read_to_string(valid_dir().join(name)).unwrap_or_else(|e| panic!("read {name}: {e}"))
}

pub fn valid_raw() -> RawCampaign {
    RawCampaign {
        world: read_valid("world.json"),
        npcs: read_valid("npcs.json"),
        classes: read_valid("classes.json"),
        quest_plan: read_valid("quest-plan.json"),
        quests: read_valid("quests.json"),
        dialogue: read_valid("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    }
}

/// The self-describing invalid-fixture format: an expected diagnostic code plus
/// a set of stage documents that wholesale replace the valid ones.
#[derive(Debug, Deserialize)]
pub struct InvalidFixture {
    pub description: String,
    pub expect: String,
    #[serde(default)]
    pub schema_reject: bool,
    pub documents: BTreeMap<String, serde_json::Value>,
}

pub fn stage_of(name: &str) -> Stage {
    match name {
        "world" => Stage::World,
        "npcs" => Stage::Npcs,
        "classes" => Stage::Classes,
        "quest-plan" => Stage::QuestPlan,
        "quests" => Stage::Quests,
        "dialogue" => Stage::Dialogue,
        "world-edits" => Stage::WorldEdits,
        other => panic!("unknown stage `{other}`"),
    }
}

/// Apply a fixture's stage overrides on top of the valid campaign.
pub fn apply(fixture: &InvalidFixture) -> RawCampaign {
    let mut raw = valid_raw();
    for (stage, doc) in &fixture.documents {
        let s = serde_json::to_string(doc).expect("re-serialize override document");
        match stage.as_str() {
            "world" => raw.world = s,
            "npcs" => raw.npcs = s,
            "classes" => raw.classes = s,
            "quest-plan" => raw.quest_plan = s,
            "quests" => raw.quests = s,
            "dialogue" => raw.dialogue = s,
            "world-edits" => raw.world_edits = Some(s),
            other => panic!("unknown stage `{other}`"),
        }
    }
    raw
}

/// Give an override document the envelope key it deliberately does not carry.
///
/// A fixture for `DW0100` is about a persona missing a field; it needs a valid
/// envelope only to get as far as its own subject, and a `dsl_version` written
/// into the file would be this engine's number restated in 37 places that have
/// nothing to say about versions. So the fixtures state none and this supplies
/// it — a fixture *cannot* hold a stale one.
///
/// A document that DOES state one keeps it: that is the one fixture whose
/// subject IS the version (`DW0102-bad-dsl-version.json`, `9.9.9`).
fn stamp(doc: &mut serde_json::Value) {
    if let Some(obj) = doc.as_object_mut()
        && !obj.contains_key("dsl_version")
    {
        obj.insert(
            "dsl_version".to_string(),
            serde_json::Value::String(delvewright_dsl::DSL_VERSION.to_string()),
        );
    }
}

/// Load every invalid fixture (sorted by filename for determinism).
pub fn load_invalid() -> Vec<(String, InvalidFixture)> {
    let mut entries: Vec<PathBuf> = fs::read_dir(invalid_dir())
        .expect("read invalid dir")
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("json"))
        .collect();
    entries.sort();
    entries
        .into_iter()
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let src = fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {name}: {e}"));
            let mut fixture: InvalidFixture =
                serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse {name}: {e}"));
            for doc in fixture.documents.values_mut() {
                stamp(doc);
            }
            (name, fixture)
        })
        .collect()
}

/// The placeholder a test document writes where the envelope's `dsl_version`
/// goes. Deliberately not version-shaped: a document that reaches the parser
/// still carrying it is refused by name (`DW0102` quotes it), never mistaken
/// for a stale number.
pub const VERSION_TOKEN: &str = "%dsl_version%";

/// A test document at the one `dsl_version` this engine accepts.
///
/// The number is [`delvewright_dsl::DSL_VERSION`] and appears in no test file:
/// a bump moves the constant and reaches every document through here. Panics
/// when the token is absent, for the same reason [`patch_doc`] parses rather
/// than splices — a `str::replace` that matches nothing returns its input
/// unchanged, and the test then asserts against a document it never stamped.
pub fn at_dsl_version(doc: &str) -> String {
    assert!(
        doc.contains(VERSION_TOKEN),
        "document carries no `{VERSION_TOKEN}` placeholder, so stamping it did \
         nothing; write the envelope key as \"dsl_version\": \"{VERSION_TOKEN}\""
    );
    doc.replace(VERSION_TOKEN, delvewright_dsl::DSL_VERSION)
}

/// Patch a JSON document **structurally**: parse the text, hand the closure the
/// parsed value, and return it re-rendered in canonical form.
///
/// See the twin in `crates/delvec/tests/common/mod.rs` for why this exists: a
/// `str::replace` that matches nothing returns its input unchanged, so a test
/// built on textual splicing goes on to assert against an **unpatched**
/// campaign and passes for the wrong reason. Canonical reformatting of the
/// fixtures exposed several such silent no-ops. A structural patch
/// panics instead.
pub fn patch_doc(text: &str, f: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut v: serde_json::Value = serde_json::from_str(text).expect("fixture is valid JSON");
    f(&mut v);
    delvewright_dsl::to_canonical_string(&v).expect("patched fixture serializes")
}
