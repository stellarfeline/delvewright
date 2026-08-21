//! `DW0450`/`DW0451` — a body may not occupy the same space as block geometry.
//!
//! The owner's island round-11 finding, in its exact shape: a
//! `minecraft:warden` actor (0.9 × 2.9 blocks) is `spawn-actor`ed at an anchor
//! whose cell — and the two cells above it — are solid rock, so the giant ships
//! embedded in the cliff face beside the cave mouth. Every other proof was
//! green, because a *walked* destination is snapped to a standable cell while a
//! *summoned* body was only ever proven to have an anchor that **resolves**.
//!
//! The fixtures build hello-world with a stage-7 edit script that reshapes the
//! room around `npc/keeper`. Doing it through the editor is deliberate: it
//! writes the geometry the shipped delve actually gets, so these also pin that
//! the proof reads the **edited** assembled world rather than the raw prefab.

mod common;

use std::collections::BTreeMap;

use delvewright_compiler::clearance::{DW_BODY_CLEARANCE, DW_BODY_CLEARANCE_ADVISORY};
use delvewright_compiler::commands::CommandTree;
use delvewright_compiler::emit::{self, BuildFailure};
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Diagnostic, RawCampaign, Severity, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The hello-world NPC document with `npc/keeper`'s body swapped for `entity`.
fn npcs_doc(base_entity: &str) -> String {
    read_hw("npcs.json").replace("\"minecraft:villager\"", &format!("\"{base_entity}\""))
}

/// A one-batch stage-7 script that fills the single cell at `offset` from
/// `anchor/keeper-stand` with stone — the smallest edit that puts real geometry
/// at a chosen place relative to a body.
fn edits_doc(offset: [i32; 3], note: &str) -> String {
    serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "stage": "world-edits",
        "content": { "batches": [ {
            "id": "batch/clearance-fixture",
            "area": "area/keep",
            "note": note,
            "edits": [
                { "verb": "select", "name": "region/beside-keeper", "shape": {
                    "kind": "box",
                    "frame": { "kind": "anchor-relative", "anchor": "anchor/keeper-stand" },
                    "min": offset, "max": offset } },
                { "verb": "fill", "region": "region/beside-keeper", "recipe": {
                    "blocks": [ { "block": "minecraft:stone", "weight": 1.0 } ] } }
            ]
        } ] }
    })
    .to_string()
}

/// Build the fixture campaign; `Ok` carries the advisory diagnostics.
fn build(base_entity: &str, edits: Option<String>) -> Result<Vec<Diagnostic>, BuildFailure> {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: npcs_doc(base_entity),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: read_hw("quests.json"),
        dialogue: read_hw("dialogue.json"),
        world_edits: edits,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
    };
    let campaign = parse_campaign(&raw).expect("campaign parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for piece in &area.pieces {
            for t in &piece.templates {
                let bytes = std::fs::read(common::prefabs_dir().join(&t.structure_file)).unwrap();
                structures.insert(t.structure_file.clone(), bytes);
            }
        }
    }
    let tree = CommandTree::v1_21_11();
    emit::build_with_warnings(
        &plan,
        &BTreeMap::new(),
        &structures,
        &tree,
        &prefabs,
        None,
        "unpinned",
        &BTreeMap::new(),
    )
    .map(|(_, warnings)| warnings)
}

/// The red fixture, and the island's mechanism: a stone block two cells above
/// the keeper's feet. A 2.9-tall warden body reaches into it — the head is in
/// the rock — so the build stops. The player's own feet+head cells are
/// untouched, so nothing else about the room changes: this is a proof about the
/// BODY's size, not about the room being broken.
#[test]
fn a_body_taller_than_its_clearance_is_dw0450() {
    let err = build(
        "minecraft:warden",
        Some(edits_doc([0, 2, 0], "stone at the giant's head height")),
    )
    .expect_err("a 2.9-tall body with 2 cells of headroom is inside the block above it");
    let BuildFailure::Diagnostic { code, message } = err else {
        panic!("expected a coded build diagnostic");
    };
    assert_eq!(code, DW_BODY_CLEARANCE, "{message}");
    assert!(
        message.contains("npc/keeper") && message.contains("minecraft:warden"),
        "the message must name the body and its entity: {message}"
    );
    assert!(
        message.contains("anchor/keeper-stand"),
        "the message must name where the body is: {message}"
    );
    assert!(
        message.contains("3 cells of headroom"),
        "the message must state how much clearance the body needs: {message}"
    );
    assert!(
        message.contains("summon"),
        "the message must explain that a `summon` does no snapping: {message}"
    );
}

/// …and the very same geometry is fine for the 1.8-tall body the room was built
/// for. The rule is the real body's size, not strictness for its own sake.
#[test]
fn the_same_geometry_fits_a_player_sized_body() {
    let warnings = build(
        "minecraft:villager",
        Some(edits_doc([0, 2, 0], "stone at the giant's head height")),
    )
    .expect("a 1.95-tall body has room under the same block");
    assert!(
        !warnings.iter().any(|d| d.code == DW_BODY_CLEARANCE),
        "a body that fits must not fail: {warnings:#?}"
    );
}

/// The green fixture: the stock hello-world room, no edits. Neither tier fires
/// — the no-false-positive guard, since `DW0451`'s 0.2-block margin must stay
/// silent for the most ordinary staging in the DSL.
#[test]
fn an_unedited_room_raises_neither_tier() {
    let warnings = build("minecraft:villager", None).expect("the stock hello-world builds");
    assert!(
        !warnings
            .iter()
            .any(|d| d.code == DW_BODY_CLEARANCE || d.code == DW_BODY_CLEARANCE_ADVISORY),
        "an ordinary NPC in an ordinary room must raise neither tier: {warnings:#?}"
    );
}

/// The advisory tier: a 0.9-wide sheep body leaves 0.05 blocks of its cell on
/// each side, so a block in the cell beside it is inside the model-overhang
/// margin — measured, reported, and the build still succeeds.
#[test]
fn a_flush_wide_body_is_a_dw0451_advisory() {
    let warnings = build(
        "minecraft:sheep",
        Some(edits_doc([1, 0, 0], "stone in the cell beside the body")),
    )
    .expect("model overhang is advisory — the build must still succeed");
    let w = warnings
        .iter()
        .find(|d| d.code == DW_BODY_CLEARANCE_ADVISORY)
        .unwrap_or_else(|| panic!("expected a DW0451 warning, got {warnings:#?}"));
    assert_eq!(w.severity, Severity::Warning);
    assert!(
        w.message.contains("0.2 blocks of it horizontally"),
        "the advisory must name the margin it measured against: {}",
        w.message
    );
    assert!(
        w.message.contains("render past their collision box"),
        "the advisory must justify itself as a model-overhang measurement: {}",
        w.message
    );
}

/// …and the identical block beside a 0.6-wide player-model body is silent: the
/// margin discriminates by how much of its own cell a body leaves free, which
/// is what keeps the tier from reporting every wall in the delve.
#[test]
fn the_same_block_beside_a_player_sized_body_is_silent() {
    let warnings = build(
        "minecraft:villager",
        Some(edits_doc([1, 0, 0], "stone in the cell beside the body")),
    )
    .expect("a 0.6-wide body beside a wall is ordinary staging");
    assert!(
        !warnings
            .iter()
            .any(|d| d.code == DW_BODY_CLEARANCE_ADVISORY),
        "a player-model body beside a wall must not warn: {warnings:#?}"
    );
}
