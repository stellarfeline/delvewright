//! A wave does not walk into a killing volume (`DW0922`), not even through a
//! barrier the party can leave open (`DW0923`).
//!
//! ## The incident (`vesperhold`)
//!
//! The Drowned Choir was seated in the Undertide Pool, nine blocks from a well
//! whose bottom is a lethal volume. A volume kills every body that is not a
//! player, so the choir drowned itself: on the pinned server a drowned stepped
//! onto a floor lantern, jumped onto the well's curb, walked along it over the
//! shut fence gate and dropped in. With the lantern moved, the same well was
//! still open through the gate a player had opened and left open, and the choir
//! re-seated after that player's death walked through it.
//!
//! ## The fixture
//!
//! `hello-world`'s keep, with the burning floor at its exit (`lethal/the-burn`,
//! the molten-stone volume `v10_lethal_volume.rs` makes legal with a side door)
//! and a wave seated at the entry. A fence line across the near hall, one cell
//! south of the keeper, shuts the entry strip off from the rest of the keep; its
//! middle cell is what each case changes, and nothing else.

mod common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure};
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;
use delvewright_dsl::{DSL_VERSION, Diagnostic, RawCampaign, parse_campaign};

fn read_hw(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

/// The hello-world quest with the sentries seated at the entry once the keeper
/// is spoken to, and the burning floor at the exit.
fn quests(follow_range: Option<f64>) -> String {
    let mut doc: serde_json::Value = serde_json::from_str(&read_hw("quests.json")).unwrap();
    doc["dsl_version"] = serde_json::json!(DSL_VERSION);
    let c = &mut doc["content"];
    c["waves"] = serde_json::json!([{
        "id": "wave/sentries",
        "anchor": "spawn",
        "mobs": [{ "entity": "minecraft:zombie", "count": 2, "name": "Sentry" }]
    }]);
    if let Some(r) = follow_range {
        c["waves"][0]["mobs"][0]["attributes"] = serde_json::json!({ "follow_range": r });
    }
    c["quests"][0]["on_objective_complete"]["obj/talk"]
        .as_array_mut()
        .expect("obj/talk carries effects")
        .push(serde_json::json!({
            "type": "spawn-wave",
            "wave": "wave/sentries",
            "happening": { "text": "wave/sentries arrives", "verb": "arrives" }
        }));
    c["lethal_volumes"] = serde_json::json!([{
        "id": "lethal/the-burn",
        "region": { "anchor": "anchor/exit", "extent": [0, 0, 0] },
        "message": "The road ends at a floor of molten stone.",
        "damage_type": "fire",
        "shown_by": ["minecraft:magma_block"]
    }]);
    doc.to_string()
}

/// The burning floor and the side door (`v10_lethal_volume.rs`'s `SIDE_DOOR`),
/// then the fence line across the entry strip with `middle` in its middle cell.
fn edits(middle: &str) -> String {
    let local = |min: [i32; 3], max: [i32; 3]| {
        serde_json::json!({
            "kind": "box",
            "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
            "min": min, "max": max
        })
    };
    let keeper = |min: [i32; 3], max: [i32; 3]| {
        serde_json::json!({
            "kind": "box",
            "frame": { "kind": "anchor-relative", "anchor": "anchor/keeper-stand" },
            "min": min, "max": max
        })
    };
    let fill = |region: &str, block: &str| {
        serde_json::json!({ "verb": "fill", "region": region,
            "recipe": { "blocks": [ { "block": block, "weight": 1.0 } ] } })
    };
    serde_json::json!({
        "dsl_version": DSL_VERSION,
        "campaign_id": "hello-world",
        "stage": "world-edits",
        "content": { "batches": [ {
            "id": "batch/the-burn-and-the-line",
            "area": "area/keep",
            "note": "The molten floor at the exit, the side door past it, and a fence line shutting the entry strip off.",
            "edits": [
                { "verb": "select", "name": "region/the-burn", "shape": local([4, 0, 7], [6, 0, 9]) },
                { "verb": "replace", "region": "region/the-burn", "matching": ["minecraft:stone"],
                  "recipe": { "blocks": [ { "block": "minecraft:magma_block", "weight": 1.0 } ] } },
                { "verb": "select", "name": "region/side-door", "shape": local([2, 1, 6], [2, 2, 6]) },
                { "verb": "replace", "region": "region/side-door", "matching": ["minecraft:stone"],
                  "recipe": { "blocks": [ { "block": "minecraft:air", "weight": 1.0 } ] } },
                { "verb": "select", "name": "region/line", "shape": keeper([-4, 0, -1], [4, 0, -1]) },
                fill("region/line", "minecraft:oak_fence"),
                { "verb": "select", "name": "region/middle", "shape": keeper([0, 0, -1], [0, 0, -1]) },
                fill("region/middle", middle)
            ]
        } ] }
    })
    .to_string()
}

fn build(middle: &str) -> Result<(emit::BuildOutput, Vec<Diagnostic>), BuildFailure> {
    build_with(middle, None)
}

fn build_with(
    middle: &str,
    follow_range: Option<f64>,
) -> Result<(emit::BuildOutput, Vec<Diagnostic>), BuildFailure> {
    let raw = RawCampaign {
        world: read_hw("world.json"),
        npcs: read_hw("npcs.json"),
        classes: read_hw("classes.json"),
        quest_plan: read_hw("quest-plan.json"),
        quests: quests(follow_range),
        dialogue: read_hw("dialogue.json"),
        world_edits: Some(edits(middle)),
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    let mut campaign = parse_campaign(&raw).expect("campaign parses");
    delvewright_dsl::tag_translatables(&mut campaign);
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
    emit::build_with_warnings(
        &plan,
        &BTreeMap::new(),
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn refusal(middle: &str) -> (String, String) {
    match build(middle) {
        Ok(_) => panic!("expected `{middle}` to be refused"),
        Err(BuildFailure::Diagnostic { code, message }) => (code.id().to_string(), message),
        Err(other) => panic!("expected a coded build diagnostic, got {other:?}"),
    }
}

/// A gap in the line: the sentries walk out of the strip, through the keep's
/// door and onto the molten floor.
#[test]
fn a_wave_that_can_walk_into_a_volume_is_dw0922() {
    let (code, message) = refusal("minecraft:air");
    assert_eq!(code, "DW0922", "{message}");
    assert!(
        message.contains("wave/sentries")
            && message.contains("minecraft:zombie")
            && message.contains("lethal/the-burn"),
        "names the wave, the body and the volume: {message}"
    );
}

/// A shut fence gate in the gap: the sentries cannot open it, but the party can,
/// and may leave it open. The perturbation only `DW0923` sees: the as-built
/// world is the one `DW0922` judged clear.
#[test]
fn a_wave_that_reaches_a_volume_through_a_gate_the_party_opens_is_dw0923() {
    let (code, message) = refusal("minecraft:oak_fence_gate[facing=north,open=false]");
    assert_eq!(code, "DW0923", "{message}");
    assert!(
        message.contains("minecraft:oak_fence_gate") && message.contains("a jump across a dry cut"),
        "names the barrier and the remedy: {message}"
    );
}

/// The same gate authored open is no barrier at all: the reach is as built.
#[test]
fn a_gate_authored_open_is_the_as_built_reach() {
    let (code, message) = refusal("minecraft:oak_fence_gate[facing=north,open=true]");
    assert_eq!(code, "DW0922", "{message}");
}

/// The same gate, with the sentries' pursuit declared short enough that even
/// through it they cannot reach the burn: the build holds, and the ledger shows
/// the opened flood was taken — one barrier a player can open, and more cells
/// reached with it open than shut.
#[test]
fn a_wave_held_short_of_the_volume_builds_and_states_its_binding() {
    let (out, _) = build_with(
        "minecraft:oak_fence_gate[facing=north,open=false]",
        Some(3.0),
    )
    .expect("the sentries pursue three blocks and the burn is further");
    let ledger: serde_json::Value = serde_json::from_slice(
        out.get("validation/wave-lethal.json")
            .expect("the wave-lethal ledger is emitted"),
    )
    .unwrap();
    assert_eq!(ledger["waves"], 1, "{ledger}");
    assert_eq!(ledger["seats"], 2, "{ledger}");
    assert_eq!(ledger["volumes"], 1, "{ledger}");
    assert_eq!(ledger["openable"], 1, "{ledger}");
    assert!(
        ledger["reached_open"].as_u64() > ledger["reached"].as_u64(),
        "the opened gate lets the flood further: {ledger}"
    );
    assert_eq!(ledger["as_built"], serde_json::json!([]), "{ledger}");
    assert_eq!(ledger["opened"], serde_json::json!([]), "{ledger}");
}
