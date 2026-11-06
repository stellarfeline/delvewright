//! **Every effect container the schema declares has a cutscene classification**
//! (`compiler::hold::Nesting`).
//!
//! The containers are enumerated from `delvec schema --stage all`, never from
//! a list here: every property of a `QuestEffect` variant whose items are a
//! `QuestEffect` or a `SequenceStep`, in every stage document that defines the
//! effect. A container the schema gains and `Nesting` does not know is red here
//! before a cutscene inside it can go unaccounted for.

use std::collections::BTreeSet;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn containers() -> (BTreeSet<String>, usize) {
    let out = Command::new(BIN)
        .args(["schema", "--stage", "all"])
        .output()
        .expect("run delvec schema");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let schema: serde_json::Value = serde_json::from_slice(&out.stdout).expect("schema is JSON");
    let mut fields = BTreeSet::new();
    let mut variants = 0;
    for doc in schema.as_object().expect("stages object").values() {
        let Some(effect) = doc
            .pointer("/$defs/QuestEffect/oneOf")
            .and_then(|v| v.as_array())
        else {
            continue;
        };
        for v in effect {
            variants += 1;
            let Some(props) = v.get("properties").and_then(|p| p.as_object()) else {
                continue;
            };
            for (name, prop) in props {
                let item = prop
                    .pointer("/items/$ref")
                    .and_then(|r| r.as_str())
                    .unwrap_or("");
                if item.ends_with("/QuestEffect") || item.ends_with("/SequenceStep") {
                    fields.insert(name.clone());
                }
            }
        }
    }
    (fields, variants)
}

#[test]
fn every_nested_effect_container_is_classified() {
    let (fields, variants) = containers();
    assert!(
        variants > 0,
        "the schema yielded no QuestEffect variant: a vacuous comparison"
    );
    let known: BTreeSet<String> = delvec::compiler::hold::Nesting::FIELDS
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        fields, known,
        "the schema's nested effect containers (left, over {variants} variant(s)) and the ones \
         `compiler::hold::Nesting` classifies (right) differ"
    );
    for f in &fields {
        // `steps` and a cutscene's `then` are timelines whose offset the carrier
        // decides (a step's `at_ticks`; the cutscene's length plus one), so
        // `hold::nested` classifies them from the effect, not from the name.
        if f != "steps" && f != "then" {
            assert!(
                delvec::compiler::hold::Nesting::of_field(f).is_some(),
                "`{f}` has no classification"
            );
        }
    }
    let cut: delvewright_dsl::QuestEffect = serde_json::from_value(serde_json::json!({
        "type": "cutscene", "seconds": 1, "path": [{"anchor": "anchor/a"}],
        "then": [{"type": "set-flag", "flag": "flag/after"}]
    }))
    .unwrap();
    let end = delvec::compiler::link::cutscene_end_offset(&cut).unwrap();
    let nested = delvec::compiler::hold::nested(&cut);
    assert_eq!(nested.len(), 1);
    assert_eq!(
        nested[0].0,
        delvec::compiler::hold::Nesting::Timeline(end + 1)
    );
}
