//! spec-0134 §4, document tier: a stack's `count` takes an integer or
//! `{base, per_player}`; the schema bounds `base ≥ 1`, restated as `DW0100`.

use delvewright_dsl::{
    DSL_VERSION, MobCount, RawCampaign, ScaledCount, Stage, parse_campaign, stage_schema,
    validate_campaign,
};
use serde_json::{Value, json};

fn hw(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/valid/hello-world")
            .join(name),
    )
    .unwrap()
}

fn with_count(count: Value) -> delvewright_dsl::Campaign {
    let mut q: Value = serde_json::from_str(&hw("quests.json")).unwrap();
    q["dsl_version"] = json!(DSL_VERSION);
    q["content"]["waves"] = json!([{"id": "wave/dusk", "anchor": "anchor/keeper-stand",
        "mobs": [{"entity": "minecraft:zombie", "count": count}]}]);
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: hw("npcs.json"),
        classes: hw("classes.json"),
        quest_plan: hw("quest-plan.json"),
        quests: q.to_string(),
        dialogue: hw("dialogue.json"),
        world_edits: None,
        geometry_brief: None,
        layout_graph: None,
        site_plan: None,
        detail_plan: None,
        design: None,
    })
    .expect("parses")
}

#[test]
fn the_schema_exports_both_count_shapes() {
    let s: Value =
        serde_json::from_str(&serde_json::to_string(&stage_schema(Stage::Quests)).unwrap())
            .unwrap();
    let defs = s["$defs"]
        .as_object()
        .or(s["definitions"].as_object())
        .unwrap();
    assert_eq!(defs["ScaledCount"]["properties"]["base"]["minimum"], 1);
    assert!(defs["MobCount"].to_string().contains("ScaledCount"));
}

#[test]
fn both_shapes_parse_and_one_player_meets_base() {
    let c = with_count(json!(3));
    assert_eq!(c.quests.content.waves[0].mobs[0].count, MobCount::Fixed(3));
    let c = with_count(json!({"base": 2, "per_player": 1}));
    let n = c.quests.content.waves[0].mobs[0].count;
    assert_eq!(
        n,
        MobCount::Scaled(ScaledCount {
            base: 2,
            per_player: 1
        })
    );
    assert_eq!((n.base(), n.max()), (2, 5));
}

#[test]
fn a_scaled_count_from_zero_is_dw0100() {
    let d = validate_campaign(&with_count(json!({"base": 0, "per_player": 2})));
    assert!(
        d.iter()
            .any(|x| x.code == "DW0100" && x.path == "/content/waves/0/mobs/0/count/base"),
        "{d:#?}"
    );
    let d = validate_campaign(&with_count(json!({"base": 1, "per_player": 2})));
    assert!(!d.iter().any(|x| x.path.ends_with("/count/base")), "{d:#?}");
}
