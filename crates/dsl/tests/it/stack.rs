//! spec-0134 §5, document tier: a rocket stack carries its stars. The
//! `fireworks` field is exported on the one stack shape every site writes (a
//! class kit, a `loot` stack, a `give-item`, a worn piece, a declared drop),
//! admitted on
//! `minecraft:firework_rocket` and refused on any other item at each site
//! (`DW1095`), and held to the bounds a `firework` effect's stars are
//! (`DW0100`). The emitted component is the compiler's, proven in
//! `crates/delvec/tests/it/stack.rs`.

use delvewright_dsl::{
    Campaign, DSL_VERSION, Diagnostic, RawCampaign, Stage, parse_campaign, stage_schema,
    validate_campaign,
};
use serde_json::{Value, json};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap_or_else(|e| panic!("hello-world/{name}: {e}"))
}

fn patched(name: &str, f: impl FnOnce(&mut Value)) -> String {
    let mut v: Value = serde_json::from_str(&hw(name)).unwrap();
    f(&mut v);
    v["dsl_version"] = json!(DSL_VERSION);
    v.to_string()
}

fn campaign(quests: impl FnOnce(&mut Value), classes: impl FnOnce(&mut Value)) -> Campaign {
    let raw = RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: patched("classes.json", classes),
        quest_plan: hw("quest-plan.json"),
        quests: patched("quests.json", quests),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    };
    parse_campaign(&raw).expect("campaign parses")
}

fn stars() -> Value {
    json!({"flight": 2, "explosions": [
        {"shape": "star", "colors": ["#ffd700"], "trail": true},
        {"shape": "burst", "colors": ["#ff0000", "#00ff00"]}
    ]})
}

fn with(code: &str, d: &[Diagnostic]) -> Vec<Diagnostic> {
    d.iter().filter(|x| x.code == code).cloned().collect()
}

/// The four stack sites a quests document holds: a `give-item` on the keeper's
/// beat, a `loot` fill (its anchor's holder is a build-tier question; the
/// stack's own questions are all answered here), an actor's worn piece and its
/// declared drop.
fn quests_with(item: &'static str, fw: Value) -> impl FnOnce(&mut Value) {
    move |q| {
        q["content"]["quests"][0]["on_objective_complete"]["obj/talk"]
            .as_array_mut()
            .unwrap()
            .push(
                json!({"type": "give-item", "item": item, "count": 3, "fireworks": fw,
                         "happening": {"text": "the keeper hands over rockets", "verb": "learns"}}),
            );
        q["content"]["loot"] = json!([{"id": "loot/stores", "anchor": "anchor/keeper-stand",
            "items": [{"item": item, "count": 2, "fireworks": fw}]}]);
        q["content"]["actors"] = json!([{
            "id": "actor/fox", "entity": "minecraft:fox", "anchor": "spawn",
            "equipment": {"off_hand": {"item": item, "fireworks": fw}},
            "drops": [{"item": item, "fireworks": fw}]
        }]);
    }
}

fn kit_with(item: &'static str, fw: Value) -> impl FnOnce(&mut Value) {
    move |c| {
        c["content"]["classes"][0]["kit"]
            .as_array_mut()
            .unwrap()
            .push(json!({"item": item, "count": 4, "fireworks": fw}));
    }
}

/// Criterion 1 and 6: the field is exported on each stack type the schema
/// carries, with the bounds stated.
#[test]
fn the_schema_exports_fireworks_on_every_stack_site() {
    let q: Value =
        serde_json::from_str(&serde_json::to_string(&stage_schema(Stage::Quests)).unwrap())
            .unwrap();
    let defs = q["$defs"]
        .as_object()
        .or(q["definitions"].as_object())
        .unwrap();
    assert!(defs["LootItem"]["properties"].get("fireworks").is_some());
    assert!(defs["Stack"]["properties"].get("fireworks").is_some());
    fn arm<'a>(v: &'a Value, tag: &str) -> Option<&'a Value> {
        match v {
            Value::Object(m) => {
                if v["properties"]["type"]["const"] == tag {
                    return Some(v);
                }
                m.values().find_map(|x| arm(x, tag))
            }
            Value::Array(a) => a.iter().find_map(|x| arm(x, tag)),
            _ => None,
        }
    }
    let give = arm(&q, "give-item").expect("give-item is exported");
    assert!(
        serde_json::to_string(give)
            .unwrap()
            .contains("\"fireworks\""),
        "the give-item arm carries `fireworks`"
    );
    let sf = &defs["StackFireworks"];
    assert_eq!(sf["properties"]["flight"]["minimum"], 1, "{sf}");
    assert_eq!(sf["properties"]["flight"]["maximum"], 3, "{sf}");
    assert_eq!(sf["properties"]["explosions"]["minItems"], 1, "{sf}");
    assert_eq!(sf["properties"]["explosions"]["maxItems"], 7, "{sf}");
    let c: Value =
        serde_json::from_str(&serde_json::to_string(&stage_schema(Stage::Classes)).unwrap())
            .unwrap();
    let cdefs = c["$defs"]
        .as_object()
        .or(c["definitions"].as_object())
        .unwrap();
    assert!(cdefs["KitItem"]["properties"].get("fireworks").is_some());
}

/// A rocket carries its stars at every site, and nothing is refused.
#[test]
fn a_rocket_with_stars_is_admitted_at_every_site() {
    let c = campaign(
        quests_with("minecraft:firework_rocket", stars()),
        kit_with("minecraft:firework_rocket", stars()),
    );
    let d = validate_campaign(&c);
    assert!(with("DW1095", &d).is_empty(), "{d:#?}");
    assert!(
        !d.iter().any(|x| x.path.contains("fireworks")),
        "nothing at a `fireworks` pointer: {d:#?}"
    );
}

/// `DW1095` at each of the five sites, at the stack's `fireworks`.
#[test]
fn stars_on_an_arrow_are_dw1095_at_each_site() {
    let c = campaign(
        quests_with("minecraft:arrow", stars()),
        kit_with("minecraft:arrow", stars()),
    );
    let d = with("DW1095", &validate_campaign(&c));
    let mut paths: Vec<&str> = d.iter().map(|x| x.path.as_str()).collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "/content/actors/0/drops/0/fireworks",
            "/content/actors/0/equipment/off_hand/fireworks",
            "/content/classes/0/kit/3/fireworks",
            "/content/loot/0/items/0/fireworks",
            "/content/quests/0/on_objective_complete/obj/talk/1/fireworks",
        ],
        "{d:#?}"
    );
    assert!(d[0].message.contains("minecraft:firework_rocket"), "{d:#?}");
}

/// The stars are held to a `firework` effect's own bounds (`DW0100`).
#[test]
fn a_stack_star_out_of_bounds_is_dw0100() {
    let bad = json!({"flight": 4, "explosions": [{"shape": "star", "colors": ["gold"]}]});
    let c = campaign(
        quests_with("minecraft:firework_rocket", bad.clone()),
        kit_with("minecraft:firework_rocket", bad),
    );
    let d = with("DW0100", &validate_campaign(&c));
    assert!(
        d.iter()
            .any(|x| x.path == "/content/classes/0/kit/3/flight"),
        "{d:#?}"
    );
    assert!(
        d.iter()
            .any(|x| x.path == "/content/loot/0/items/0/explosions/0/colors/0"),
        "{d:#?}"
    );
}
