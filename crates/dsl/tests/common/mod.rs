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
            let fixture: InvalidFixture =
                serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse {name}: {e}"));
            (name, fixture)
        })
        .collect()
}

/// Patch a JSON document **structurally**: parse the text, hand the closure the
/// parsed value, and return it re-rendered in canonical form.
///
/// See the twin in `crates/compiler/tests/common/mod.rs` for why this exists: a
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
