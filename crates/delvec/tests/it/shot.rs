//! spec-0134 §2/§3 — a target block is shot, and an item is raised. Proven
//! over hello-world in a synthetic stone room (`water_route`'s, dry): a
//! `shot` trigger at `anchor/exit` places its target there, owes a bow and
//! arrows (`DW1090`) and a stance that sees the target (`DW1091`), and is
//! dispatched by a `target_hit` advancement bounded to the target's cell; a
//! `use-item` trigger is dispatched by a `using_item` advancement with the
//! item and the box.

use crate::common;

use std::collections::BTreeMap;

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildFailure, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{Diagnostic, RawCampaign, parse_campaign, validate_campaign_with};
use serde_json::{Value, json};

fn read(name: &str) -> String {
    std::fs::read_to_string(common::hello_world_dir().join(name)).unwrap()
}

fn pal_entry(name: &str) -> fastnbt::Value {
    let mut c = std::collections::HashMap::new();
    c.insert("Name".to_string(), fastnbt::Value::String(name.to_string()));
    fastnbt::Value::Compound(c)
}

/// An 11×6×11 stone room, lit, with `walls` stone cells inside it.
fn room(walls: &[(i32, i32, i32)]) -> Vec<u8> {
    use fastnbt::Value;
    let (sx, sy, sz) = (11i32, 6i32, 11i32);
    let mut cells: BTreeMap<(i32, i32, i32), i32> = BTreeMap::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                if y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1 {
                    cells.insert((x, y, z), 1);
                }
            }
        }
    }
    for w in walls {
        cells.insert(*w, 1);
    }
    for x in [2, sx / 2, sx - 3] {
        for z in [2, sz / 2, sz - 3] {
            cells.insert((x, sy - 2, z), 2);
        }
    }
    let blocks: Vec<Value> = cells
        .into_iter()
        .map(|((x, y, z), state)| {
            let mut c = std::collections::HashMap::new();
            c.insert(
                "pos".to_string(),
                Value::List(vec![Value::Int(x), Value::Int(y), Value::Int(z)]),
            );
            c.insert("state".to_string(), Value::Int(state));
            Value::Compound(c)
        })
        .collect();
    let mut root = std::collections::HashMap::new();
    root.insert("DataVersion".to_string(), Value::Int(4671));
    root.insert(
        "size".to_string(),
        Value::List(vec![Value::Int(sx), Value::Int(sy), Value::Int(sz)]),
    );
    root.insert(
        "palette".to_string(),
        Value::List(vec![
            pal_entry("minecraft:air"),
            pal_entry("minecraft:stone"),
            pal_entry("minecraft:glowstone"),
        ]),
    );
    root.insert("blocks".to_string(), Value::List(blocks));
    root.insert("entities".to_string(), Value::List(vec![]));
    let raw = fastnbt::to_bytes(&Value::Compound(root)).unwrap();
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut gz, &raw).unwrap();
    gz.finish().unwrap()
}

/// hello-world with a `shot` at the exit and a `use-item` at the keeper,
/// then `mutate` over (classes, quests).
fn docs(mutate: impl FnOnce(&mut Value, &mut Value)) -> (Value, Value) {
    let mut classes: Value = serde_json::from_str(&read("classes.json")).unwrap();
    classes["content"]["classes"][0]["kit"]
        .as_array_mut()
        .unwrap()
        .extend([
            json!({"item": "minecraft:bow", "count": 1}),
            json!({"item": "minecraft:arrow", "count": 16}),
            json!({"item": "minecraft:spyglass", "count": 1}),
        ]);
    let mut q: Value = serde_json::from_str(&read("quests.json")).unwrap();
    let t = q["content"]
        .as_object_mut()
        .unwrap()
        .entry("triggers")
        .or_insert(json!([]));
    t.as_array_mut().unwrap().extend([
        json!({"id": "trigger/hit-the-mark", "at": "anchor/exit",
               "on": {"on": "shot"}, "prop": {"block": "minecraft:target"},
               "effects": [{"type": "narrate", "text": "A clean hit.", "style": "chat"}]}),
        json!({"id": "trigger/look-out", "at": "anchor/keeper-stand",
               "on": {"on": "use-item", "item": "minecraft:spyglass", "range": 2},
               "effects": [{"type": "narrate", "text": "Far off, a sail.", "style": "chat"}]}),
    ]);
    mutate(&mut classes, &mut q);
    (classes, q)
}

fn campaign(classes: &Value, q: &Value) -> delvewright_dsl::Campaign {
    parse_campaign(&RawCampaign {
        world: read("world.json"),
        npcs: read("npcs.json"),
        classes: classes.to_string(),
        quest_plan: read("quest-plan.json"),
        quests: q.to_string(),
        dialogue: read("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("parses")
}

fn diags(classes: &Value, q: &Value) -> Vec<Diagnostic> {
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    validate_campaign_with(
        &campaign(classes, q),
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    )
}

fn codes(d: &[Diagnostic], code: &str) -> Vec<String> {
    d.iter()
        .filter(|x| x.code == code)
        .map(|x| x.path.clone())
        .collect()
}

fn build(
    classes: &Value,
    q: &Value,
    walls: &[(i32, i32, i32)],
) -> Result<BuildOutput, BuildFailure> {
    let c = campaign(classes, q);
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let plan = Plan::build(&c, &prefabs).expect("plan builds");
    let nbt = room(walls);
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for area in &plan.areas {
        for t in area.pieces.iter().flat_map(|p| &p.templates) {
            structures.insert(t.structure_file.clone(), nbt.clone());
        }
    }
    emit::build(
        &plan,
        &load_campaign_dir(&common::hello_world_dir())
            .unwrap()
            .inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
}

fn json_at(out: &BuildOutput, path: &str) -> Value {
    serde_json::from_slice(out.get(path).unwrap_or_else(|| panic!("no {path}"))).unwrap()
}

#[test]
fn a_shot_and_a_raised_item_validate() {
    let (c, q) = docs(|_, _| {});
    let d = diags(&c, &q);
    assert!(codes(&d, "DW1090").is_empty(), "{d:#?}");
    assert!(codes(&d, "DW0143").is_empty(), "{d:#?}");
}

#[test]
fn a_shot_with_no_prop_or_another_block_is_dw1090() {
    let (c, q) = docs(|_, q| {
        q["content"]["triggers"][0]
            .as_object_mut()
            .unwrap()
            .remove("prop");
    });
    assert_eq!(codes(&diags(&c, &q), "DW1090"), vec!["/content/triggers/0"]);
    let (c, q) = docs(|_, q| {
        q["content"]["triggers"][0]["prop"]["block"] = json!("minecraft:hay_block");
    });
    assert_eq!(
        codes(&diags(&c, &q), "DW1090"),
        vec!["/content/triggers/0/prop/block"]
    );
}

#[test]
fn a_raised_item_outside_the_registry_is_dw0143() {
    let (c, q) = docs(|_, q| {
        q["content"]["triggers"][1]["on"]["item"] = json!("minecraft:telescope");
    });
    assert_eq!(
        codes(&diags(&c, &q), "DW0143"),
        vec!["/content/triggers/1/on/item"]
    );
}

#[test]
fn the_shot_and_the_raised_item_are_dispatched_by_their_own_criteria() {
    let (c, q) = docs(|_, _| {});
    let out = build(&c, &q, &[]).expect("builds");
    let shot = json_at(
        &out,
        "datapack/data/hello-world/advancement/press_hit_the_mark.json",
    );
    assert_eq!(shot["criteria"]["act"]["trigger"], "minecraft:target_hit");
    let pos = &shot["criteria"]["act"]["conditions"]["projectile"][0]["predicate"]["location"]["position"];
    // The exit anchor is world [5, 65, 8]; the cell grown by half a block.
    assert_eq!(pos["x"], json!({"min": 4.5, "max": 6.5}));
    assert_eq!(pos["y"], json!({"min": 64.5, "max": 66.5}));
    assert_eq!(
        shot["rewards"]["function"],
        "hello-world:press_hit_the_mark"
    );
    let raised = json_at(
        &out,
        "datapack/data/hello-world/advancement/press_look_out.json",
    );
    assert_eq!(raised["criteria"]["act"]["trigger"], "minecraft:using_item");
    assert_eq!(
        raised["criteria"]["act"]["conditions"]["item"]["items"],
        "minecraft:spyglass"
    );
    // The setup places the target.
    let all: String = out
        .iter()
        .filter(|(k, _)| k.ends_with(".mcfunction"))
        .map(|(_, v)| String::from_utf8_lossy(v).to_string())
        .collect();
    assert!(all.contains("setblock 5 65 8 minecraft:target"));
    // The press dispatch revokes and fires the bundle.
    let press = String::from_utf8(
        out["datapack/data/hello-world/function/press_hit_the_mark.mcfunction"].clone(),
    )
    .unwrap();
    assert!(press.starts_with("advancement revoke @s only hello-world:press_hit_the_mark"));
    assert!(press.contains("function hello-world:trig_hit_the_mark"));
}

#[test]
fn a_shot_with_no_bow_in_any_hand_is_dw1090() {
    let (c, q) = docs(|c, _| {
        c["content"]["classes"][0]["kit"]
            .as_array_mut()
            .unwrap()
            .retain(|k| k["item"] != "minecraft:bow");
    });
    match build(&c, &q, &[]) {
        Err(BuildFailure::Diagnostic { code, message }) => {
            assert_eq!(code, "DW1090", "{message}");
            assert!(message.contains("a bow or a crossbow"), "{message}");
        }
        other => panic!("expected DW1090, got {other:?}"),
    }
}

// `DW1091` over a walled hall, and the stance seeing the centre by the sight
// rule's own walk, are `nav::shot`'s unit tests: a refusal here would need
// a world the critical path still crosses, which this room cannot hold.

#[test]
fn a_raised_item_the_game_never_holds_in_use_is_dw1092() {
    let (c, q) = docs(|_, q| {
        q["content"]["triggers"][1]["on"]["item"] = json!("minecraft:snowball");
    });
    let d = diags(&c, &q);
    assert_eq!(codes(&d, "DW1092"), vec!["/content/triggers/1/on/item"]);
    assert!(codes(&d, "DW0143").is_empty(), "{d:#?}");
    // The spyglass is held in use; the primary is clean of the code.
    let (c, q) = docs(|_, _| {});
    assert!(codes(&diags(&c, &q), "DW1092").is_empty());
}

#[test]
fn the_use_duration_table_is_the_pinned_games() {
    use delvewright_dsl::trigger::item_use_duration;
    // Read from the jar by `tools/maintenance/extract-item-use-duration.py`.
    assert_eq!(item_use_duration("minecraft:spyglass"), Some(1200));
    assert_eq!(item_use_duration("minecraft:bow"), Some(72000));
    assert_eq!(item_use_duration("minecraft:goat_horn"), Some(140));
    assert_eq!(item_use_duration("minecraft:snowball"), None);
    assert_eq!(item_use_duration("minecraft:stone"), None);
}
