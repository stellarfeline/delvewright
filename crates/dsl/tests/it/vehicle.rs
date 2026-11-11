//! spec-0135 criteria 1 and 2: the vehicle surface, the steering table, and
//! the kind the engine derives from an entity.

use delvewright_dsl::{VehicleKind, VehicleMedium, steering, steering_table};

/// Criterion 1: `vehicles[]` is exported with its eight fields, and the two
/// verbs with it.
#[test]
fn the_surface_is_exported() {
    let schema = delvewright_dsl::schema::stage_schema(delvewright_dsl::envelope::Stage::Quests);
    let defs = &schema["$defs"];
    let mut keys: Vec<&str> = defs["Vehicle"]["properties"]
        .as_object()
        .expect("Vehicle has properties")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "at",
            "attributes",
            "bounds",
            "entity",
            "equipment",
            "facing",
            "id",
            "name"
        ]
    );
    let verbs: Vec<String> = defs["QuestEffect"]["oneOf"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|b| {
            b["properties"]["type"]["const"]
                .as_str()
                .map(str::to_string)
        })
        .collect();
    assert!(verbs.iter().any(|v| v == "seat-vehicle"));
    assert!(verbs.iter().any(|v| v == "despawn-vehicle"));
}

/// Criterion 1: every objective kind takes `ride`, and a census's `of` takes
/// `riders`.
#[test]
fn every_objective_kind_takes_a_ride_and_a_census_counts_riders() {
    let schema = delvewright_dsl::schema::stage_schema(delvewright_dsl::envelope::Stage::Quests);
    let defs = &schema["$defs"];
    let kinds: Vec<(&str, bool)> = defs["Objective"]["oneOf"]
        .as_array()
        .expect("Objective is a union")
        .iter()
        .map(|b| {
            (
                b["properties"]["type"]["const"].as_str().unwrap_or("?"),
                b["properties"].get("ride").is_some(),
            )
        })
        .collect();
    assert_eq!(kinds.len(), 5, "{kinds:?}");
    assert!(kinds.iter().all(|(_, ride)| *ride), "{kinds:?}");
    let of = serde_json::to_string(&defs["CensusOf"]).unwrap();
    assert!(of.contains("\"riders\""), "{of}");
}

/// Criterion 1: the steering table lists the kinds the pinned jar steers,
/// with their items and seats.
#[test]
fn the_steering_table_holds_the_pinned_game() {
    let t = steering_table();
    let count = |k: VehicleKind| t.values().filter(|s| s.kind == k).count();
    assert_eq!(count(VehicleKind::Boat), 10);
    assert_eq!(count(VehicleKind::Minecart), 1);
    assert_eq!(count(VehicleKind::Mount), 11);
    let pig = steering("minecraft:pig").unwrap();
    assert_eq!(
        pig.steering_item.as_deref(),
        Some("minecraft:carrot_on_a_stick")
    );
    assert_eq!(pig.seats, 1);
    assert_eq!(
        steering("strider").unwrap().steering_item.as_deref(),
        Some("minecraft:warped_fungus_on_a_stick")
    );
    assert_eq!(steering("minecraft:oak_boat").unwrap().seats, 2);
    assert!(steering("minecraft:horse").unwrap().tame);
    assert!(!pig.tame);
}

/// Criterion 2: a boat, a raft, the minecart and each steerable kind resolve
/// their kind; a chest boat, a hopper cart, a llama and a zombie carry no
/// rider.
#[test]
fn the_kind_is_derived_from_the_entity() {
    assert_eq!(
        steering("minecraft:oak_boat").unwrap().kind,
        VehicleKind::Boat
    );
    assert_eq!(
        steering("minecraft:bamboo_raft").unwrap().kind,
        VehicleKind::Boat
    );
    assert_eq!(
        steering("minecraft:minecart").unwrap().kind,
        VehicleKind::Minecart
    );
    for m in [
        "pig",
        "strider",
        "horse",
        "donkey",
        "mule",
        "skeleton_horse",
        "zombie_horse",
        "camel",
    ] {
        let s = steering(m).unwrap_or_else(|| panic!("{m}"));
        assert_eq!(s.kind, VehicleKind::Mount, "{m}");
        assert_eq!(s.medium, VehicleMedium::Ground, "{m}");
    }
    for none in [
        "minecraft:oak_chest_boat",
        "minecraft:hopper_minecart",
        "minecraft:chest_minecart",
        "minecraft:llama",
        "minecraft:zombie",
    ] {
        assert!(steering(none).is_none(), "{none}");
    }
    assert_eq!(
        steering("minecraft:nautilus").unwrap().medium,
        VehicleMedium::Water
    );
}
