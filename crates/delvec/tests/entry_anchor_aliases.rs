//! **One concept, two spellings — and every consumer resolves both.**
//!
//! `plan::ENTRY_ANCHOR_NAMES` (`spawn`, then `entry`) exists because the shipped
//! tileset library spells one thing two ways: the keep/cave/test generators write
//! `spawn`, the island generator writes `entry`. Its own doc comment says the
//! compiler owns that resolution "rather than leaving it to downstream folklore",
//! and the gate-deadlock proof repeats the claim in its own words — *the same
//! alias list every other consumer uses*.
//!
//! Three consumers did not. Inter-area transport, the POV shot planner and the
//! trap-safety start set each matched the literal string `spawn`, so an area whose
//! entry anchor is spelled `entry` — every island-tileset area in the shipped
//! library — was never transported into, never framed, and never counted as a
//! place a player can start from. Nothing errored: the lookup asked an honest
//! question about the wrong key and got an honest `None`.
//!
//! These tests bind to the *behaviour*, not to the call sites, so a fourth
//! consumer written the same way is caught by the same red.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildOutput};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::parse_campaign;
use serde_json::Value;

/// The landing's entry anchor in world coordinates — the destination the bolt
/// branch's crossing has to land on, whichever way the anchor is spelled.
const LANDING_SPAWN: [i32; 3] = [262, 66, 8];

fn fixture_dir() -> PathBuf {
    common::compiler_fixtures_dir().join("branch-transport")
}

/// A private copy of the prefab library in which `cave-shore` — the piece
/// `area/landing` binds — spells its entry anchor the island tileset's way.
///
/// The rename is the whole perturbation: same cell, same facing, same piece, same
/// campaign. Only the key changes, from `spawn` to `entry`.
fn prefabs_spelling_entry(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-entry-alias-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("cave-shore.json");
    let mut meta: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let anchors = meta["anchors"].as_object_mut().unwrap();
    let spawn = anchors
        .remove("spawn")
        .expect("fixture drift: cave-shore must declare a `spawn` anchor to rename");
    anchors.insert("entry".to_string(), spawn);
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

/// Run `f` over a plan built against `prefabs_dir` (a `Plan` borrows its
/// campaign, so it cannot outlive one).
fn with_plan<T>(prefabs_dir: &Path, f: impl FnOnce(&Plan) -> T) -> T {
    let loaded = delvewright_compiler::load::load_campaign_dir(&fixture_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    f(&plan)
}

fn build_with(prefabs_dir: &Path) -> BuildOutput {
    let loaded = delvewright_compiler::load::load_campaign_dir(&fixture_dir()).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("valid campaign parses");
    let prefabs = PrefabRegistry::load_dir(prefabs_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(prefabs_dir.join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .expect("emission succeeds")
}

/// The `transport` cell the bolt branch's crossing promises, read off the
/// exported branch path — the artifact the harness actually walks.
fn bolt_transport(out: &BuildOutput) -> Option<Value> {
    let raw = out.get("validation/branch-path-bolt.json")?;
    let doc: Value = serde_json::from_slice(raw).ok()?;
    doc["steps"]
        .as_array()?
        .iter()
        .find(|s| s["objective"] == "obj/bolt")
        .and_then(|s| s.get("transport").cloned())
}

/// **The control that keeps the case below from being vacuously green.** With the
/// library as shipped — `cave-shore` spelling its entry anchor `spawn` — the
/// crossing is found. Without this, "the rename still works" would prove nothing:
/// a fixture that never crossed at all would pass a `!= None` assertion the same
/// way it would fail one.
#[test]
fn the_shipped_spelling_is_transported_into() {
    let out = build_with(&common::prefabs_dir());
    assert_eq!(
        bolt_transport(&out),
        Some(serde_json::json!(LANDING_SPAWN)),
        "fixture drift: the bolt branch must cross into area/landing at obj/bolt"
    );
}

/// The binding case: the SAME campaign, the same cell, the same crossing — with
/// the destination area's entry anchor spelled the island tileset's way. An entry
/// anchor is an entry anchor whichever alias names it, so the party is still
/// carried across.
#[test]
fn the_island_spelling_is_transported_into() {
    let dir = prefabs_spelling_entry("transport");
    let out = build_with(&dir);
    assert_eq!(
        bolt_transport(&out),
        Some(serde_json::json!(LANDING_SPAWN)),
        "an area whose entry anchor is spelled `entry` must still be transported into"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The resolution itself, stated once and directly: `Plan::entry_point` is the
/// one place that knows the spellings, and it answers with the same cell for
/// both. Every consumer is required to go through it — this is what they get.
#[test]
fn entry_point_resolves_either_spelling_to_one_cell() {
    let dir = prefabs_spelling_entry("resolver");
    let a = with_plan(&common::prefabs_dir(), |p| {
        p.entry_point("area/landing")
            .expect("the shipped library spells it `spawn`")
    });
    let b = with_plan(&dir, |p| {
        p.entry_point("area/landing")
            .expect("the renamed library spells it `entry`")
    });
    assert_eq!(a, b, "one concept, two spellings, one cell");
    assert_eq!(a, LANDING_SPAWN);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The resolution is a **sweep** as well as a lookup, because one consumer (the
/// trap-safety start set) wants every cell a player can start from rather than
/// one area's. It goes through the same resolver, so both spellings are counted
/// and an ordinary anchor never is.
#[test]
fn the_start_set_is_every_area_entry_and_nothing_else() {
    let dir = prefabs_spelling_entry("sweep");
    let shipped = with_plan(&common::prefabs_dir(), |p| {
        p.entry_points().collect::<Vec<_>>()
    });
    let renamed = with_plan(&dir, |p| p.entry_points().collect::<Vec<_>>());
    assert_eq!(
        shipped, renamed,
        "the start set does not depend on how a piece spells its entry anchor"
    );
    assert!(
        shipped.contains(&LANDING_SPAWN),
        "the landing's entry is a place a player can start from: {shipped:?}"
    );
    assert_eq!(
        shipped.len(),
        2,
        "one start per area, and this campaign has two: {shipped:?}"
    );
    let exit = with_plan(&common::prefabs_dir(), |p| {
        p.point("area/landing", "anchor/exit")
            .expect("fixture drift: `cave-shore` declares an exit anchor")
    });
    assert!(
        !shipped.contains(&exit),
        "an ordinary anchor is not a start: {shipped:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
