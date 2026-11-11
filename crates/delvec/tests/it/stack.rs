//! spec-0134 §5, the compiler's half: a rocket stack's stars are emitted as the
//! `minecraft:fireworks` component at every stack site — one test per site, and
//! the sites enumerated from the exported schema's stack type, so a site the
//! schema gains is a site this file owes a test. Proven on the built
//! hello-world: a `give-item`, a class kit, an actor's worn piece and its
//! declared drop; a `loot` fill in the renderer's unit tests (`emit::loot`),
//! which hello-world's anchors cannot seat. The `firework` effect's summoned
//! rocket is rendered by the same function, so a star means one thing
//! everywhere.

use crate::common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::commands::CommandTree;
use delvec::compiler::emit::{self, BuildOutput};
use delvec::compiler::load::load_campaign_dir;
use delvec::compiler::plan::Plan;
use delvec::compiler::registry::{FullEntityRegistry, FullItemRegistry, PrefabRegistry};
use delvewright_dsl::{parse_campaign, validate_campaign_with};
use serde_json::json;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-stack-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn build(dir: &Path) -> BuildOutput {
    let loaded = load_campaign_dir(dir).unwrap();
    let campaign = parse_campaign(&loaded.raw).expect("fixture parses");
    let prefabs = PrefabRegistry::load_dir(&common::prefabs_dir()).unwrap();
    let diags = validate_campaign_with(
        &campaign,
        &FullItemRegistry::v1_21_11(),
        &prefabs,
        &FullEntityRegistry::v1_21_11(),
    );
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == delvewright_dsl::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "the fixture must validate: {errors:#?}");
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
    emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &CommandTree::v1_21_11(),
        &prefabs,
        None,
        &BTreeMap::new(),
    )
    .unwrap_or_else(|e| panic!("the fixture builds: {e:?}"))
}

/// Every function line of the build containing `needle`.
fn lines_with(out: &BuildOutput, needle: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (path, bytes) in out.iter() {
        if !path.starts_with("datapack/") || !path.ends_with(".mcfunction") {
            continue;
        }
        for l in String::from_utf8_lossy(bytes).lines() {
            if l.contains(needle) {
                found.push(l.to_string());
            }
        }
    }
    found
}

/// The stars the fixture declares, and the component value they render to.
const STARS_VALUE: &str =
    "fireworks={flight_duration:2b,explosions:[{shape:\"star\",colors:[I;16766720],has_trail:1b}]}";

/// The schema's `$defs` that carry a stack: a definition whose own body (not
/// following a reference) holds an object with every key of the stack, or a
/// reference to the stack itself.
fn stack_bearing_defs() -> Vec<String> {
    let keys = delvewright_dsl::STACK_FIELDS;
    fn holds(v: &serde_json::Value, keys: &[&str]) -> bool {
        match v {
            serde_json::Value::Object(m) => {
                if m.get("$ref").and_then(|r| r.as_str()) == Some("#/$defs/Stack") {
                    return true;
                }
                if let Some(p) = m.get("properties").and_then(|p| p.as_object())
                    && keys.iter().all(|k| p.contains_key(*k))
                {
                    return true;
                }
                m.values().any(|x| holds(x, keys))
            }
            serde_json::Value::Array(a) => a.iter().any(|x| holds(x, keys)),
            _ => false,
        }
    }
    // A verb arm carries its stack inline (the verb flattens it): named by
    // its `type`.
    fn verb_arms(
        v: &serde_json::Value,
        keys: &[&str],
        out: &mut std::collections::BTreeSet<String>,
    ) {
        match v {
            serde_json::Value::Object(m) => {
                if let Some(tag) = m
                    .get("properties")
                    .and_then(|p| p.get("type"))
                    .and_then(|t| t.get("const"))
                    .and_then(|c| c.as_str())
                    && holds(v, keys)
                {
                    out.insert(format!("verb {tag}"));
                }
                m.values().for_each(|x| verb_arms(x, keys, out));
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| verb_arms(x, keys, out)),
            _ => {}
        }
    }
    let mut found = std::collections::BTreeSet::new();
    for stage in delvewright_dsl::Stage::ALL {
        let schema = serde_json::to_value(delvewright_dsl::stage_schema(stage)).unwrap();
        if let Some(defs) = schema.get("$defs").and_then(|d| d.as_object()) {
            for (name, def) in defs {
                if name != "Stack" && def.get("oneOf").is_none() && holds(def, keys) {
                    found.insert(name.clone());
                }
                verb_arms(def, keys, &mut found);
            }
        }
    }
    found.into_iter().collect()
}

/// **The sites are the schema's**: every definition that carries the stack is
/// one of the five this file proves, and each of the five is carried — a sixth
/// is a test this file owes.
#[test]
fn the_stack_sites_are_enumerated_from_the_schema() {
    assert_eq!(
        stack_bearing_defs(),
        vec![
            "EquipItem",
            "KitItem",
            "LootItem",
            "MobDrop",
            "verb give-item"
        ],
        "a stack site was added or lost: give it its test below"
    );
    assert_eq!(delvewright_dsl::StackSiteKind::ALL.len(), 5);
}

/// hello-world with an elite fox set out at the keeper's talk, carrying a
/// starred rocket in its mouth (the main hand, the one slot a fox draws) and
/// declaring two as its drop.
fn with_fox(dir: &Path) {
    let stars = json!({"flight": 2, "explosions": [
        {"shape": "star", "colors": ["#ffd700"], "trail": true}
    ]});
    common::patch_file(&dir.join("quests.json"), move |q| {
        q["content"]["actors"] = json!([{
            "id": "actor/fox", "entity": "minecraft:fox", "anchor": "spawn",
            "offset": [2, 0, 0], "facing": "south", "tier": "elite", "vulnerable": true,
            "equipment": {"main_hand": {"item": "minecraft:firework_rocket", "fireworks": stars}},
            "drops": [{"item": "minecraft:firework_rocket", "count": 2, "fireworks": stars}]
        }]);
        q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(json!({"type": "spawn-actor", "actor": "actor/fox",
                         "happening": {"verb": "arrives", "text": "the fox is set out"}}));
    });
}

/// The worn piece: the actor's summon carries the component in its NBT.
#[test]
fn a_worn_rocket_carries_its_stars() {
    let dir = scratch("worn");
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    with_fox(&dir);
    let out = build(&dir);
    let summons = lines_with(&out, "summon minecraft:fox");
    assert!(
        summons.iter().any(|l| l.contains(
            "mainhand:{id:\"minecraft:firework_rocket\",count:1,components:{\"minecraft:fireworks\":{flight_duration:2b,explosions:[{shape:\"star\",colors:[I;16766720],has_trail:1b}]}}}"
        )),
        "{summons:#?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The drop: the actor's death loot table sets the component.
#[test]
fn a_dropped_rocket_carries_its_stars() {
    let dir = scratch("drop");
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    with_fox(&dir);
    let out = build(&dir);
    let (_, table) = out
        .iter()
        .find(|(k, _)| k.contains("/loot_table/") && k.contains("fox"))
        .unwrap_or_else(|| panic!("the fox's drop table is emitted"));
    let v: serde_json::Value = serde_json::from_slice(table).unwrap();
    let entry = &v["pools"][0]["entries"][0];
    assert_eq!(entry["name"], "minecraft:firework_rocket", "{v}");
    let fns = entry["functions"].as_array().unwrap();
    assert_eq!(
        fns[0],
        json!({"function": "minecraft:set_count", "count": 2})
    );
    let fw = &fns[1]["components"]["minecraft:fireworks"];
    assert_eq!(
        *fw,
        json!({"flight_duration": 2, "explosions": [
            {"shape": "star", "colors": [16766720], "has_trail": true}
        ]}),
        "{v}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_rocket_given_and_kitted_carries_its_stars() {
    let dir = scratch("sites");
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    let stars = json!({"flight": 2, "explosions": [
        {"shape": "star", "colors": ["#ffd700"], "trail": true}
    ]});
    let fw = stars.clone();
    common::patch_file(&dir.join("quests.json"), move |q| {
        q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(
                json!({"type": "give-item", "item": "minecraft:firework_rocket",
                         "count": 3, "fireworks": fw,
                         "happening": {"text": "the keeper hands over rockets", "verb": "learns"}}),
            );
    });
    common::patch_file(&dir.join("classes.json"), move |c| {
        c["content"]["classes"][0]["kit"]
            .as_array_mut()
            .unwrap()
            .push(json!({"item": "minecraft:firework_rocket", "count": 4, "fireworks": stars}));
    });
    let out = build(&dir);
    let lines = lines_with(&out, "minecraft:firework_rocket[");
    let give = lines
        .iter()
        .filter(|l| {
            l.contains(&format!(
                "give @a minecraft:firework_rocket[{STARS_VALUE}] 3"
            ))
        })
        .count();
    let kit = lines
        .iter()
        .filter(|l| {
            l.contains(&format!(
                "give @s minecraft:firework_rocket[{STARS_VALUE}] 4"
            ))
        })
        .count();
    assert_eq!(give, 1, "the give-item stack: {lines:#?}");
    assert_eq!(kit, 1, "the kit stack: {lines:#?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A stack with no `fireworks` is byte-identical to the emission before the
/// field: no `fireworks=` anywhere in hello-world's build.
#[test]
fn no_stars_no_component() {
    let dir = scratch("none");
    common::copy_dir_all(&common::hello_world_dir(), &dir);
    let out = build(&dir);
    assert!(lines_with(&out, "fireworks=").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
