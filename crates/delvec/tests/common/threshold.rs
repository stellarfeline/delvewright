//! The chapel-door fixture for `DW1044` (`tests/threshold.rs`, and its remedy
//! rows in `tests/remedy_reachability.rs`): `hello-world`, whose keep has an
//! inner wall at piece-local z 6 with a two-wide doorway (`anchor/door`,
//! x 4..5), opened by the Keeper, and an environment trigger at `at` that pays
//! the flag the exit objective reads.

use serde_json::{Value, json};
use std::path::Path;

/// Write the fixture into `camp`. `on` is the trigger's `on` object; `edits`,
/// when given, is the one world-edits batch over the keep.
pub fn doorway_campaign(camp: &Path, on: Value, at: &str, edits: Option<Vec<Value>>) {
    super::copy_dir_all(&super::hello_world_dir(), camp);
    super::patch_file(&camp.join("quests.json"), |q| {
        let quest = &mut q["content"]["quests"][0];
        quest["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({ "type": "set-flag", "flag": "flag/door-open" }));
        quest["objectives"][1]["requires_flags"] = json!(["flag/through-the-door"]);
        q["content"]["triggers"] = json!([{
            "id": "trigger/through-the-door",
            "at": at,
            "on": on,
            "requires_flags": ["flag/door-open"],
            "effects": [{ "type": "set-flag", "flag": "flag/through-the-door" }]
        }]);
    });
    if let Some(edits) = edits {
        std::fs::write(
            camp.join("world-edits.json"),
            serde_json::to_string_pretty(&json!({
                "campaign_id": "hello-world",
                "content": { "batches": [{
                    "id": "batch/the-wall",
                    "area": "area/keep",
                    "note": "The keep's inner wall, opened or closed beside its doorway.",
                    "edits": edits
                }] },
                "dsl_version": delvewright_dsl::DSL_VERSION,
                "stage": "world-edits"
            }))
            .unwrap(),
        )
        .unwrap();
    }
}

/// An `approach` with `range`.
pub fn approach(range: u32) -> Value {
    json!({ "on": "approach", "range": range })
}

/// A piece-local box over the inner wall at `x`, foot and head height.
pub fn wall_cell(name: &str, x: [i32; 2]) -> Value {
    json!({
        "verb": "select",
        "name": name,
        "shape": {
            "kind": "box",
            "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
            "min": [x[0], 1, 6],
            "max": [x[1], 2, 6]
        }
    })
}

/// A second doorway through the inner wall at x 8, away from the trigger.
pub fn second_door() -> Vec<Value> {
    vec![
        wall_cell("region/second-door", [8, 8]),
        json!({ "verb": "carve", "region": "region/second-door" }),
    ]
}

/// [`second_door`], walled again.
pub fn second_door_closed() -> Vec<Value> {
    let mut edits = second_door();
    edits.push(json!({
        "verb": "fill",
        "region": "region/second-door",
        "recipe": { "blocks": [{ "block": "minecraft:stone_bricks", "weight": 1.0 }], "scale": 1.0 }
    }));
    edits
}
