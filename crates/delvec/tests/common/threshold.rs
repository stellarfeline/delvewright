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

/// The flag the trigger pays moved off `obj/exit` onto a later beat,
/// `obj/look-back`, back at the Keeper's post, and the trigger armed by the
/// exit's completion: `obj/exit` is armed from the talk on, so its title and
/// hint guide the walk to the exit (`DW0863`), the trigger on it is performed
/// once the exit is done, and the path still owes it before the last beat.
pub fn gate_a_later_beat(camp: &Path) {
    super::patch_file(&camp.join("quests.json"), |q| {
        q["content"]["quests"][0]["on_objective_complete"]["obj/exit"] =
            json!([{ "type": "set-flag", "flag": "flag/out-the-door" }]);
        q["content"]["triggers"][0]["requires_flags"] = json!(["flag/out-the-door"]);
        let objectives = q["content"]["quests"][0]["objectives"]
            .as_array_mut()
            .unwrap();
        let gate = objectives[1]
            .as_object_mut()
            .unwrap()
            .remove("requires_flags")
            .expect("the exit carries the trigger's flag");
        objectives.push(json!({
            "type": "reach-anchor",
            "id": "obj/look-back",
            "anchor": "anchor/keeper-stand",
            "radius": 2,
            "after": ["obj/exit"],
            "requires_flags": gate,
            "title": "Go back to the keeper",
            "hint": "Back through the doorway, to his post.",
            "happening": { "text": "the party completes obj/look-back", "verb": "arrives" }
        }));
    });
}

/// The Keeper's door left barred: `obj/talk` no longer opens it.
///
/// A door the story shuts with a second doorway carved beside it is a way
/// round a shut gate, which `DW1011` refuses whatever the trigger does. With
/// [`second_door`] open, the door is one the story never operates instead,
/// and the second doorway is the way through.
pub fn bar_the_door(camp: &Path) {
    super::patch_file(&camp.join("quests.json"), |q| {
        let talk = q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap();
        let before = talk.len();
        talk.retain(|e| !(e["type"] == "open-gate" && e["anchor"] == "anchor/door"));
        assert_eq!(
            before - talk.len(),
            1,
            "exactly one open-gate on the door is removed"
        );
    });
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
