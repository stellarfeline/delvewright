//! **A watch belongs to a body** (spec-0101 §1): `watch` is one type,
//! declared on both body classes and nowhere else, and `body_watch_sites` walks
//! every declaration.
//!
//! The schema is read the way `body_skin_sites.rs` reads it — generated from
//! the Rust types by `schemars`, never from a list written here — so a third
//! class declaring a `watch` of its own, or a second watch type, reds this file.

mod common;

use std::collections::BTreeSet;

use delvewright_dsl::BodyRef;
use delvewright_dsl::envelope::Stage;
use serde_json::{Value, json};

/// Every `(schema class, $ref target)` pair whose `properties.watch` exists,
/// across every stage schema, and every `$defs` name a `watch` resolves to.
#[allow(clippy::type_complexity)]
fn watch_declarations() -> (
    BTreeSet<String>,
    BTreeSet<String>,
    Value,
    BTreeSet<(String, String)>,
) {
    let mut wrappers = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut targets = BTreeSet::new();
    let mut def = Value::Null;
    for stage in Stage::ALL {
        let schema = delvewright_dsl::stage_schema(stage);
        let Some(defs) = schema.get("$defs").and_then(Value::as_object) else {
            continue;
        };
        for (name, d) in defs {
            let Some(w) = d.get("properties").and_then(|p| p.get("watch")) else {
                continue;
            };
            classes.insert(name.clone());
            // `Option<T>` is written as `anyOf: [{$ref}, {type: null}]` or as a
            // bare `$ref`; either way the definition is the one `$ref`.
            let refs: Vec<String> = std::iter::once(w)
                .chain(
                    w.get("anyOf")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten(),
                )
                .filter_map(|v| v.get("$ref").and_then(Value::as_str))
                .map(|r| r.trim_start_matches("#/$defs/").to_string())
                .collect();
            assert_eq!(refs.len(), 1, "`{name}.watch` names one definition: {w}");
            // An NPC's `watch` is `NpcWatch`: the shared watch, or the keyword
            // `"none"` its default makes meaningful. Followed one hop to the
            // watch it wraps, so the shared type is still what both classes
            // name.
            let mut target = refs[0].clone();
            if let Some(branches) = defs
                .get(&target)
                .and_then(|t| t.get("anyOf"))
                .and_then(Value::as_array)
            {
                let wrapped: Vec<String> = branches
                    .iter()
                    .filter_map(|v| v.get("$ref").and_then(Value::as_str))
                    .map(|r| r.trim_start_matches("#/$defs/").to_string())
                    .filter(|r| {
                        defs.get(r)
                            .and_then(|d| d.get("properties"))
                            .is_some_and(|p| p.get("who").is_some())
                    })
                    .collect();
                assert_eq!(wrapped.len(), 1, "`{target}` wraps one watch: {branches:?}");
                wrappers.insert((name.clone(), target.clone()));
                target = wrapped[0].clone();
            }
            if let Some(t) = defs.get(&target) {
                def = t.clone();
            }
            targets.insert(target);
        }
    }
    (classes, targets, def, wrappers)
}

#[test]
fn watch_is_one_type_on_both_body_classes_and_nowhere_else() {
    let (classes, targets, def, wrappers) = watch_declarations();
    // Only the NPC wraps it: the default watch, and so its `"none"`, belong to
    // the body a player can talk to.
    assert_eq!(
        wrappers,
        BTreeSet::from([("Npc".to_string(), "NpcWatch".to_string())]),
        "only an NPC's watch admits `none`"
    );
    let expected: BTreeSet<String> = BodyRef::ALL_CLASSES.iter().map(|s| s.to_string()).collect();
    println!(
        "watch binding: {} schema class(es) declare `watch` ({classes:?}), resolving to {targets:?}",
        classes.len()
    );
    assert_eq!(
        classes, expected,
        "exactly the body classes declare `watch`"
    );
    // Vacuity: two declarations of the same shape would pass the class check;
    // the definition both sites reference must be ONE.
    assert_eq!(
        targets,
        BTreeSet::from(["BodyWatch".to_string()]),
        "both sites reference one schema definition"
    );
    let props = def["properties"]
        .as_object()
        .expect("BodyWatch has properties");
    assert!(
        props.contains_key("who") && props.contains_key("within"),
        "{def}"
    );
    let required: BTreeSet<&str> = def["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        required,
        BTreeSet::from(["who", "within"]),
        "no defaults: {def}"
    );
    assert_eq!(def["additionalProperties"], json!(false));
}

#[test]
fn a_watch_on_a_wave_mob_is_an_unknown_field() {
    let quests = common::patch_doc(&common::read_valid("quests.json"), |d| {
        d["content"]["waves"] = json!([{
            "id": "wave/guards",
            "anchor": "anchor/keeper-stand",
            "mobs": [{
                "count": 1,
                "entity": "minecraft:zombie",
                "watch": { "who": "nearest", "within": 4 }
            }]
        }]);
    });
    let raw = delvewright_dsl::RawCampaign {
        quests,
        ..common::valid_raw()
    };
    let diags = delvewright_dsl::parse_campaign(&raw).expect_err("a wave mob declares no watch");
    assert!(
        diags
            .iter()
            .any(|d| d.code == "DW0100" && d.message.contains("unknown field `watch`")),
        "{diags:?}"
    );
}

#[test]
fn body_watch_sites_walks_every_watching_body_of_every_class() {
    let npcs = common::patch_doc(&common::read_valid("npcs.json"), |d| {
        d["content"]["npcs"][0]["watch"] = json!({ "who": "nearest", "within": 8 });
    });
    let quests = common::patch_doc(&common::read_valid("quests.json"), |d| {
        d["content"]["actors"] = json!([
            { "id": "actor/watcher", "entity": "minecraft:zombie", "anchor": "anchor/exit",
              "watch": { "who": { "class": "class/wanderer" }, "within": 3 } },
            { "id": "actor/blind", "entity": "minecraft:zombie", "anchor": "anchor/exit" }
        ]);
    });
    let raw = delvewright_dsl::RawCampaign {
        npcs,
        quests,
        ..common::valid_raw()
    };
    let c = delvewright_dsl::parse_campaign(&raw).expect("campaign parses");
    let sites = delvewright_dsl::body_watch_sites(&c);
    let seen: Vec<(&str, &str, String, u32)> = sites
        .iter()
        .map(|s| {
            (
                s.body.class(),
                s.body.id(),
                s.watch.who.token(),
                s.watch.within.get(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        vec![
            ("Npc", "npc/keeper", "nearest".to_string(), 8),
            ("Actor", "actor/watcher", "class/wanderer".to_string(), 3),
        ]
    );
    assert_eq!(sites[0].path, "/content/npcs/0/watch");
    assert_eq!(sites[1].path, "/content/actors/0/watch");
}
