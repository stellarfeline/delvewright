//! DSL v0.83, document tier: a body's vanilla variant — one field on every
//! class that summons a body, judged against the pinned table (`DW1131`,
//! `DW1132`), and written by one emission rule.

use delvewright_dsl::{
    BodyVariant, DSL_VERSION, RawCampaign, body_variant_sites, parse_campaign, render_fields,
    validate_campaign, variant_fields, variant_nbt, variant_properties, variant_table,
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

/// hello-world with `npcs` and `quests` content edited.
fn with(
    npcs: impl FnOnce(&mut Value),
    quests: impl FnOnce(&mut Value),
) -> delvewright_dsl::Campaign {
    let mut n: Value = serde_json::from_str(&hw("npcs.json")).unwrap();
    n["dsl_version"] = json!(DSL_VERSION);
    npcs(&mut n["content"]);
    let mut q: Value = serde_json::from_str(&hw("quests.json")).unwrap();
    q["dsl_version"] = json!(DSL_VERSION);
    quests(&mut q["content"]);
    parse_campaign(&RawCampaign {
        world: hw("world.json"),
        npcs: n.to_string(),
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

fn codes_at(c: &delvewright_dsl::Campaign, code: &str) -> Vec<String> {
    validate_campaign(c)
        .into_iter()
        .filter(|d| d.code == code)
        .map(|d| d.path)
        .collect()
}

fn var(pairs: &[(&str, &str)]) -> BodyVariant {
    BodyVariant(
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    )
}

/// One body of every class, each declaring `variant`.
fn every_class(variant: Value) -> delvewright_dsl::Campaign {
    let v1 = variant.clone();
    with(
        |n| {
            n["npcs"][0]["base_entity"] = json!("minecraft:cat");
            n["npcs"][0]["variant"] = v1;
        },
        |q| {
            q["actors"] = json!([{"id": "actor/tom", "entity": "minecraft:cat",
                "anchor": "anchor/keeper-stand", "variant": variant}]);
            q["waves"] = json!([{"id": "wave/cats", "anchor": "anchor/keeper-stand",
                "mobs": [{"entity": "minecraft:cat", "count": 1, "variant": variant,
                          "mount": {"entity": "minecraft:cat", "variant": variant}}]}]);
            q["vehicles"] = json!([{"id": "vehicle/cat", "entity": "minecraft:cat",
                "at": {"anchor": "anchor/keeper-stand"},
                "bounds": {"anchor": "anchor/keeper-stand", "extent": [8, 3, 8]},
                "variant": variant}]);
        },
    )
}

#[test]
fn every_class_that_summons_a_body_declares_its_variant_through_one_field() {
    let c = every_class(json!({"variant": "minecraft:tabby", "collar": "blue"}));
    let sites = body_variant_sites(&c);
    let got: Vec<(&str, String)> = sites.iter().map(|s| (s.what, s.path.clone())).collect();
    assert_eq!(
        got,
        [
            ("npc", "/content/npcs/0/variant".to_string()),
            ("actor", "/content/actors/0/variant".to_string()),
            ("wave-mob", "/content/waves/0/mobs/0/variant".to_string()),
            ("mount", "/content/waves/0/mobs/0/mount/variant".to_string()),
            ("vehicle", "/content/vehicles/0/variant".to_string()),
        ]
    );
    for code in ["DW1131", "DW1132"] {
        assert!(
            codes_at(&c, code).is_empty(),
            "{code}: {:?}",
            codes_at(&c, code)
        );
    }
}

#[test]
fn a_property_the_body_does_not_carry_is_dw1131_in_every_class() {
    let c = every_class(json!({"markings": "white"}));
    assert_eq!(
        codes_at(&c, "DW1131"),
        [
            "/content/npcs/0/variant/markings",
            "/content/actors/0/variant/markings",
            "/content/waves/0/mobs/0/variant/markings",
            "/content/waves/0/mobs/0/mount/variant/markings",
            "/content/vehicles/0/variant/markings",
        ]
    );
    // A type that carries no property at all is refused the same way.
    let zombie = with(
        |n| {
            n["npcs"][0]["base_entity"] = json!("minecraft:zombie");
            n["npcs"][0]["variant"] = json!({"variant": "minecraft:tabby"});
        },
        |_| {},
    );
    let d: Vec<_> = validate_campaign(&zombie)
        .into_iter()
        .filter(|d| d.code == "DW1131")
        .collect();
    assert_eq!(d.len(), 1, "{d:#?}");
    assert!(
        d[0].message.contains("carries no variant property"),
        "{}",
        d[0].message
    );
}

#[test]
fn a_skinned_body_is_a_mannequin_and_carries_no_variant() {
    let c = with(
        |n| {
            n["npcs"][0]["base_entity"] = json!("minecraft:cat");
            n["npcs"][0]["skin"] = json!({"texture_id": "keeper", "model": "wide"});
            n["npcs"][0]["variant"] = json!({"variant": "minecraft:tabby"});
        },
        |_| {},
    );
    assert_eq!(codes_at(&c, "DW1131"), ["/content/npcs/0/variant/variant"]);
}

#[test]
fn a_value_the_property_does_not_take_is_dw1132() {
    let c = every_class(json!({"variant": "minecraft:ashen", "collar": "teal"}));
    let got = codes_at(&c, "DW1132");
    // Two per body: a wolf's coat on a cat, and no dye named teal.
    assert_eq!(got.len(), 10, "{got:?}");
    assert!(got.contains(&"/content/vehicles/0/variant/collar".to_string()));
    assert!(got.contains(&"/content/npcs/0/variant/variant".to_string()));
    let d = validate_campaign(&c)
        .into_iter()
        .find(|d| d.code == "DW1132" && d.path == "/content/npcs/0/variant/variant")
        .unwrap();
    assert!(d.message.contains("`minecraft:tabby`"), "{}", d.message);
}

#[test]
fn an_empty_variant_declares_nothing_and_writes_nothing() {
    let c = every_class(json!({}));
    assert_eq!(body_variant_sites(&c).len(), 5);
    assert!(codes_at(&c, "DW1131").is_empty());
    assert_eq!(variant_nbt("minecraft:cat", Some(&var(&[]))), "");
    assert_eq!(variant_nbt("minecraft:cat", None), "");
}

#[test]
fn the_emission_rule_writes_each_form_the_table_states() {
    // A registry id as a string; a dye as a byte id.
    assert_eq!(
        variant_nbt(
            "minecraft:cat",
            Some(&var(&[("variant", "minecraft:tabby"), ("collar", "blue")]))
        ),
        ",CollarColor:11b,variant:\"minecraft:tabby\""
    );
    // Two properties packed into one int: colour in the low byte, markings above.
    assert_eq!(
        variant_nbt(
            "horse",
            Some(&var(&[("variant", "black"), ("markings", "white_dots")]))
        ),
        format!(",Variant:{}", 4 | (3 << 8))
    );
    // Three into one: the pattern's packed id, then the two dyes.
    assert_eq!(
        variant_nbt(
            "minecraft:tropical_fish",
            Some(&var(&[
                ("pattern", "betty"),
                ("base_color", "blue"),
                ("pattern_color", "red")
            ]))
        ),
        format!(",Variant:{}", 1025 | (11 << 16) | (14 << 24))
    );
    // A field of a compound.
    assert_eq!(
        variant_nbt(
            "minecraft:villager",
            Some(&var(&[
                ("variant", "minecraft:taiga"),
                ("profession", "minecraft:librarian")
            ]))
        ),
        ",VillagerData:{profession:\"minecraft:librarian\",type:\"minecraft:taiga\"}"
    );
    // One property written to two keys.
    assert_eq!(
        variant_nbt("minecraft:panda", Some(&var(&[("gene", "brown")]))),
        ",HiddenGene:\"brown\",MainGene:\"brown\""
    );
    // An int id that is not a list index.
    assert_eq!(
        render_fields(&variant_fields(
            "minecraft:rabbit",
            &var(&[("variant", "evil")])
        )),
        "RabbitType:99"
    );
}

#[test]
fn the_table_is_the_pinned_games_own() {
    let t = variant_table();
    assert_eq!(t.entities.len(), 23);
    let cat = variant_properties("minecraft:cat");
    assert_eq!(
        cat.keys().copied().collect::<Vec<_>>(),
        ["collar", "variant"]
    );
    assert_eq!(
        cat["variant"].component.as_deref(),
        Some("minecraft:cat/variant")
    );
    assert_eq!(t.domains["cat_variant"].len(), 11);
    assert!(variant_properties("minecraft:zombie").is_empty());
    // Every property's domain exists and is non-empty; every numeric slot's
    // domain gives every value an id.
    for (e, props) in &t.entities {
        for (p, prop) in props {
            let d = &t.domains[&prop.domain];
            assert!(!d.is_empty(), "{e}.{p}");
            for s in &prop.nbt {
                if s.form != delvewright_dsl::VariantForm::String {
                    assert!(d.iter().all(|v| v.id.is_some()), "{e}.{p}");
                }
            }
        }
    }
}
