//! Exported JSON Schemas accept every valid fixture and reject the schema-level
//! invalid fixtures (proves the LLM-facing schema matches the Rust truth).

mod common;

use delvewright_dsl::{Stage, stage_schema};

fn assert_valid_against_stage(name: &str, stage: Stage) {
    let src = common::read_valid(name);
    let instance: serde_json::Value = serde_json::from_str(&src).unwrap();
    let schema = stage_schema(stage);
    let validator = jsonschema::validator_for(&schema).expect("compile schema");
    if let Err(err) = validator.validate(&instance) {
        panic!("valid fixture {name} rejected by its schema: {err}");
    }
}

#[test]
fn schemas_accept_valid_fixtures() {
    assert_valid_against_stage("world.json", Stage::World);
    assert_valid_against_stage("npcs.json", Stage::Npcs);
    assert_valid_against_stage("classes.json", Stage::Classes);
    assert_valid_against_stage("quest-plan.json", Stage::QuestPlan);
    assert_valid_against_stage("quests.json", Stage::Quests);
}

#[test]
fn schemas_reject_schema_level_invalid_fixtures() {
    let fixtures = common::load_invalid();
    let mut checked = 0;
    for (name, fixture) in &fixtures {
        if !fixture.schema_reject {
            continue;
        }
        checked += 1;
        let mut any_rejected = false;
        for (stage_name, doc) in &fixture.documents {
            let stage = common::stage_of(stage_name);
            let schema = stage_schema(stage);
            let validator = jsonschema::validator_for(&schema).expect("compile schema");
            if !validator.is_valid(doc) {
                any_rejected = true;
            }
        }
        assert!(
            any_rejected,
            "schema-level fixture {name} was not rejected by any stage schema"
        );
    }
    assert!(
        checked > 0,
        "no schema-level invalid fixtures were exercised"
    );
}
