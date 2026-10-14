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
        geometry_brief: Some(read_valid("geometry-brief.json")),
        layout_graph: Some(read_valid("layout-graph.json")),
        site_plan: Some(read_valid("site-plan.json")),
        detail_plan: Some(read_valid("detail-plan.json")),
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
        "geometry-brief" => Stage::GeometryBrief,
        "layout-graph" => Stage::LayoutGraph,
        "site-plan" => Stage::SitePlan,
        "detail-plan" => Stage::DetailPlan,
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
            "geometry-brief" => raw.geometry_brief = Some(s),
            "layout-graph" => raw.layout_graph = Some(s),
            "site-plan" => raw.site_plan = Some(s),
            "detail-plan" => raw.detail_plan = Some(s),
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

/// [`delvewright_dsl::check_campaign`] over a fixture, less the two advisories
/// every one-place site plan carries whatever its subject: `DW0822` (the
/// critical path's projected minutes, which refuses nothing) and `DW0813` (a
/// building metric the gym has not walked). Each has its own test; a fixture
/// about something else would otherwise be "clean" of nothing.
pub fn check_campaign(raw: &RawCampaign) -> Vec<delvewright_dsl::Diagnostic> {
    let mut raw = raw.clone();
    sync_map(&mut raw);
    delvewright_dsl::check_campaign(&raw)
        .into_iter()
        .filter(|d| !(d.code == "DW0822" || d.code == "DW0813"))
        .collect()
}

/// **The map follows the story it was handed** — for a fixture whose layout
/// graph and detail plan are still the hello-world's own, the procedural half
/// of both documents is re-derived from the stage documents the test supplied:
/// one beat per objective on the one place, one station per anchor name the
/// documents reference (kept as declared where it already was; a new name a
/// gate verb opens or closes is a `gate`, any other a `point`), and each new
/// station bound to the piece anchor of the same name. A test that writes its
/// own graph or detail plan is judged on exactly what it wrote.
pub fn sync_map(raw: &mut RawCampaign) {
    let fixture_graph = read_valid("layout-graph.json");
    let fixture_detail = read_valid("detail-plan.json");
    if raw.layout_graph.as_deref() != Some(fixture_graph.as_str()) {
        return;
    }
    let mut graph: serde_json::Value = serde_json::from_str(&fixture_graph).unwrap();
    let node = graph["content"]["nodes"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let Ok(quests) = serde_json::from_str::<serde_json::Value>(&raw.quests) else {
        return;
    };
    let mut beats = Vec::new();
    if let Some(qs) = quests["content"]["quests"].as_array() {
        for q in qs {
            for o in q["objectives"].as_array().into_iter().flatten() {
                beats.push(serde_json::json!({
                    "node": node, "objective": o["id"], "quest": q["id"]
                }));
            }
        }
    }
    graph["content"]["beats"] = serde_json::Value::Array(beats);
    let docs = [
        raw.world.as_str(),
        raw.npcs.as_str(),
        raw.quests.as_str(),
        raw.dialogue.as_str(),
        raw.quest_plan.as_str(),
        raw.world_edits.as_deref().unwrap_or(""),
    ];
    let anchor = regex_lite_anchors(&docs.join("\n"));
    let gates = gate_anchors(&docs);
    let stations = graph["content"]["nodes"][0]["stations"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut by: BTreeMap<String, serde_json::Value> = stations
        .into_iter()
        .map(|s| (s["anchor"].as_str().unwrap().to_string(), s))
        .collect();
    // Only a name the bound piece declares: a name nothing answers stays
    // unresolved, as a typo must.
    let piece: serde_json::Value =
        serde_json::from_str(include_str!("../../data/anchors.json")).unwrap();
    let declared: std::collections::BTreeSet<String> = piece["prefab/hello-room"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let mut added = Vec::new();
    for a in anchor {
        if !declared.contains(&a) {
            continue;
        }
        if !by.contains_key(&a) {
            let kind = if gates.contains(&a) { "gate" } else { "point" };
            by.insert(a.clone(), serde_json::json!({"anchor": a, "kind": kind}));
            added.push(a);
        }
    }
    graph["content"]["nodes"][0]["stations"] = serde_json::Value::Array(by.into_values().collect());
    raw.layout_graph = Some(serde_json::to_string(&graph).unwrap());
    if raw.detail_plan.as_deref() == Some(fixture_detail.as_str()) && !added.is_empty() {
        let mut d: serde_json::Value = serde_json::from_str(&fixture_detail).unwrap();
        for a in &added {
            d["content"]["details"][0]["anchors"][a] = serde_json::json!(a);
        }
        raw.detail_plan = Some(serde_json::to_string(&d).unwrap());
    }
}

/// Every `anchor/<kebab>` string a set of documents names.
fn regex_lite_anchors(text: &str) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    let bytes = text.as_bytes();
    let pat = b"\"anchor/";
    let mut i = 0;
    while let Some(off) = text[i..].find("\"anchor/") {
        let start = i + off + 1;
        let mut j = start + pat.len() - 1;
        while j < bytes.len()
            && (bytes[j].is_ascii_lowercase() || bytes[j].is_ascii_digit() || bytes[j] == b'-')
        {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == b'"' && j > start + 7 {
            out.insert(text[start..j].to_string());
        }
        i = j;
    }
    out
}

/// Every anchor an `open-gate` or `close-gate` effect names.
fn gate_anchors(docs: &[&str]) -> std::collections::BTreeSet<String> {
    let mut out = std::collections::BTreeSet::new();
    fn walk(v: &serde_json::Value, out: &mut std::collections::BTreeSet<String>) {
        match v {
            serde_json::Value::Object(m) => {
                if matches!(
                    m.get("type").and_then(|t| t.as_str()),
                    Some("open-gate" | "close-gate")
                ) && let Some(a) = m.get("anchor").and_then(|a| a.as_str())
                {
                    out.insert(a.to_string());
                }
                m.values().for_each(|x| walk(x, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    for doc in docs {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(doc) {
            walk(&v, &mut out);
        }
    }
    out
}

/// [`delvewright_dsl::parse_campaign`] after [`sync_map`].
pub fn parse_campaign(
    raw: &RawCampaign,
) -> Result<delvewright_dsl::Campaign, Vec<delvewright_dsl::Diagnostic>> {
    let mut raw = raw.clone();
    sync_map(&mut raw);
    delvewright_dsl::parse_campaign(&raw)
}

/// [`delvewright_dsl::validate_campaign`] less the two one-place advisories
/// [`check_campaign`] names.
pub fn validate_campaign(c: &delvewright_dsl::Campaign) -> Vec<delvewright_dsl::Diagnostic> {
    delvewright_dsl::validate_campaign(c)
        .into_iter()
        .filter(|d| !(d.code == "DW0822" || d.code == "DW0813"))
        .collect()
}

/// A diagnostic list less the two one-place advisories [`check_campaign`]
/// names.
pub fn quiet(d: Vec<delvewright_dsl::Diagnostic>) -> Vec<delvewright_dsl::Diagnostic> {
    d.into_iter()
        .filter(|d| !(d.code == "DW0822" || d.code == "DW0813"))
        .collect()
}
