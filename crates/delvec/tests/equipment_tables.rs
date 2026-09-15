//! The two pinned tables the equipment fit rule (`DW0898`, spec-0067) reads,
//! held to the game's own data.
//!
//! - `crates/delvec/data/item-equippable-1.21.11.json` — the item half,
//!   extracted by `tools/extract-item-equippable.py`;
//! - `crates/dsl/data/entity-slots-1.21.11.json` — the body table, authored
//!   from the pinned client's renderers.
//!
//! No data file the game ships says which entity types carry a humanoid armour
//! layer or a hand layer, so the body table has no machine proof of
//! visibility. What the game's data does say is checked here every day: the
//! saddle wearers are the saddle tag, the body wearers are the item allowed
//! lists plus a named excess, and the body/saddle layer types are the ones the
//! equipment assets declare.

use std::collections::{BTreeMap, BTreeSet};

use delvec::compiler::registry::{FullEntityRegistry, ItemEquippableTable};
use delvewright_dsl::equipment::{BodyRow, admitted_entities, body_table};
use delvewright_dsl::{EntityRegistry, EquipSlot, PieceKind};

fn items() -> ItemEquippableTable {
    ItemEquippableTable::v1_21_11()
}

/// **The vocabulary is the game's eight slots** (spec-0067 criterion 1), the
/// values the `minecraft:equippable` `slot` field takes per the Minecraft Wiki
/// page *Data component format/equippable* for Java 1.21.11.
#[test]
fn the_slot_vocabulary_is_the_eight_the_game_names() {
    let wiki: BTreeSet<&str> = [
        "head", "chest", "legs", "feet", "body", "mainhand", "offhand", "saddle",
    ]
    .into_iter()
    .collect();
    let ours: BTreeSet<&str> = EquipSlot::ALL.iter().map(|s| s.nbt()).collect();
    assert_eq!(EquipSlot::ALL.len(), 8);
    assert_eq!(ours, wiki);
    let eq = delvewright_dsl::MobEquipment {
        head: None,
        chest: None,
        legs: None,
        feet: None,
        main_hand: None,
        off_hand: None,
        body: None,
        saddle: None,
    };
    assert_eq!(eq.slots().len(), EquipSlot::ALL.len());
    for s in EquipSlot::ALL {
        assert_eq!(EquipSlot::from_nbt(s.nbt()), Some(s));
    }
}

/// **The registry cross-check** (criterion 2): every `slot` value the pinned
/// item data uses is in the vocabulary, with the counts spec-0067 §1 and §4.1
/// state.
#[test]
fn every_slot_the_item_data_declares_is_in_the_vocabulary() {
    let t = items();
    let mut per_slot: BTreeMap<String, usize> = BTreeMap::new();
    let mut kinds: BTreeMap<PieceKind, usize> = BTreeMap::new();
    let mut allowed = 0usize;
    for (id, e) in &t.items {
        assert!(
            e.declared_slot().is_some(),
            "{id} declares the slot `{}`, which EquipSlot::ALL does not have",
            e.slot
        );
        *per_slot.entry(e.slot.clone()).or_default() += 1;
        *kinds.entry(e.kind).or_default() += 1;
        if !e.allowed_entities.is_empty() {
            allowed += 1;
        }
    }
    println!(
        "item equippable binding: {} item(s) over {} slot value(s): {per_slot:?}; {allowed} with an \
         allowed-entity list; kinds {kinds:?}",
        t.items.len(),
        per_slot.len()
    );
    let want: BTreeMap<String, usize> = [
        ("body", 44),
        ("head", 16),
        ("chest", 8),
        ("feet", 7),
        ("legs", 7),
        ("saddle", 1),
        ("offhand", 1),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    assert_eq!(t.items.len(), 84);
    assert_eq!(per_slot, want);
    assert_eq!(allowed, 45);
    let want_kinds: BTreeMap<PieceKind, usize> = [
        (PieceKind::Armour, 29),
        (PieceKind::Wings, 1),
        (PieceKind::Animal, 45),
        (PieceKind::Item, 9),
    ]
    .into_iter()
    .collect();
    assert_eq!(kinds, want_kinds);
}

/// The group a row falls in, by the slots it draws (spec-0067 §4.1's table).
fn group(row: &BorrowedRow) -> &'static str {
    let has = |s: &str| row.slots.contains(s);
    if has("legs") {
        "humanoid six"
    } else if has("head") && has("offhand") {
        "head item and both hands"
    } else if has("head") {
        "head item and main hand"
    } else if has("mainhand") && has("offhand") {
        "both hands only"
    } else if has("mainhand") {
        "main hand only"
    } else if has("body") && has("saddle") {
        "body and saddle"
    } else if has("saddle") {
        "saddle only"
    } else if has("body") {
        "body only"
    } else {
        "nothing"
    }
}

struct BorrowedRow {
    slots: BTreeSet<String>,
}

fn borrowed(row: &BodyRow) -> BorrowedRow {
    BorrowedRow {
        slots: row.slots.keys().cloned().collect(),
    }
}

/// **The body table** (criterion 4): one row per living entity type of the
/// pinned registry, each naming its renderer, in the groups and per-slot counts
/// spec-0067 §4.1 states.
#[test]
fn the_body_table_has_a_row_per_living_entity_type_in_the_stated_groups() {
    let table = body_table();
    let registry: Vec<String> =
        serde_json::from_str(include_str!("../data/entities-1.21.11.json")).unwrap();
    let entities = FullEntityRegistry::v1_21_11();
    for (id, row) in table {
        assert!(entities.contains(id), "{id} is not a pinned entity type");
        assert!(
            row.renderer.ends_with("Renderer"),
            "{id} names no renderer class: `{}`",
            row.renderer
        );
        for (slot, shown) in &row.slots {
            assert!(
                EquipSlot::from_nbt(slot).is_some(),
                "{id} draws `{slot}`, which is not a slot of the game"
            );
            assert!(!shown.kinds.is_empty(), "{id} `{slot}` draws no kind");
        }
    }
    println!(
        "body table binding: {} row(s) over {} pinned entity type(s)",
        table.len(),
        registry.len()
    );
    assert_eq!(registry.len(), 157);
    assert_eq!(table.len(), 92);

    let mut groups: BTreeMap<&str, usize> = BTreeMap::new();
    for row in table.values() {
        *groups.entry(group(&borrowed(row))).or_default() += 1;
    }
    let want: BTreeMap<&str, usize> = [
        ("humanoid six", 16),
        ("head item and both hands", 5),
        ("head item and main hand", 2),
        ("main hand only", 4),
        ("both hands only", 2),
        ("body and saddle", 5),
        ("saddle only", 6),
        ("body only", 4),
        ("nothing", 48),
    ]
    .into_iter()
    .collect();
    assert_eq!(groups, want);

    let mut per_slot: BTreeMap<&str, usize> = BTreeMap::new();
    for row in table.values() {
        for s in EquipSlot::ALL {
            if row.shown(s).is_some() {
                *per_slot.entry(s.nbt()).or_default() += 1;
            }
        }
    }
    let want: BTreeMap<&str, usize> = [
        ("mainhand", 29),
        ("offhand", 23),
        ("head", 23),
        ("chest", 16),
        ("legs", 16),
        ("feet", 16),
        ("saddle", 11),
        ("body", 9),
    ]
    .into_iter()
    .collect();
    assert_eq!(per_slot, want);

    // Head: 16 draw armour, 22 draw an item; the giant draws armour alone.
    let head = |k: PieceKind| {
        table
            .values()
            .filter(|r| {
                r.shown(EquipSlot::Head)
                    .is_some_and(|s| s.kinds.contains(&k))
            })
            .count()
    };
    assert_eq!(head(PieceKind::Armour), 16);
    assert_eq!(head(PieceKind::Item), 22);

    // The four bodies whose hand is drawn only in a state.
    let conditional: BTreeSet<&str> = table
        .iter()
        .filter(|(_, r)| r.slots.values().any(|s| s.when.is_some()))
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(
        conditional,
        [
            "minecraft:evoker",
            "minecraft:illusioner",
            "minecraft:panda",
            "minecraft:vindicator"
        ]
        .into_iter()
        .collect()
    );
}

fn rows_drawing(slot: EquipSlot) -> BTreeSet<String> {
    body_table()
        .iter()
        .filter(|(_, r)| r.shown(slot).is_some())
        .map(|(id, _)| id.clone())
        .collect()
}

/// **Cross-check (a)** (criterion 5): the saddle rows are exactly vanilla's
/// `#minecraft:can_equip_saddle`.
#[test]
fn the_saddle_rows_are_the_saddle_tag() {
    let rows = rows_drawing(EquipSlot::Saddle);
    let tag: BTreeSet<String> = delvewright_dsl::registry::entity_tags()
        .get("minecraft:can_equip_saddle")
        .cloned()
        .unwrap_or_default();
    println!(
        "saddle cross-check binding: {} saddle row(s), {} tag member(s), of {} row(s)",
        rows.len(),
        tag.len(),
        body_table().len()
    );
    assert_eq!(rows.len(), 11);
    assert_eq!(rows, tag);
}

/// **Cross-check (b)** (criterion 5): the body rows contain every entity a
/// `body` item admits, and the excess is exactly the one body no item admits.
#[test]
fn the_body_rows_are_the_body_items_allowed_lists_plus_the_skeleton_horse() {
    let rows = rows_drawing(EquipSlot::Body);
    let t = items();
    let body_items: Vec<_> = t
        .items
        .values()
        .filter(|e| e.declared_slot() == Some(EquipSlot::Body))
        .collect();
    let mut admitted: BTreeSet<String> = BTreeSet::new();
    for e in &body_items {
        admitted.extend(admitted_entities(&e.allowed_entities));
    }
    println!(
        "body cross-check binding: {} body row(s); {} entity type(s) admitted by {} body item(s)",
        rows.len(),
        admitted.len(),
        body_items.len()
    );
    assert!(
        admitted.is_subset(&rows),
        "admitted but not drawn: {:?}",
        admitted.difference(&rows).collect::<Vec<_>>()
    );
    let excess: Vec<&String> = rows.difference(&admitted).collect();
    assert_eq!(excess, vec!["minecraft:skeleton_horse"]);
}

/// **Cross-check (c)** (criterion 5): the body/saddle layer types the rows name
/// are exactly the ones the pinned equipment assets declare.
#[test]
fn the_rows_layer_types_are_the_assets_layer_types() {
    let t = items();
    let named: BTreeSet<String> = body_table()
        .values()
        .flat_map(|r| r.slots.values().filter_map(|s| s.layer.clone()))
        .collect();
    let assets: BTreeSet<String> = t
        .asset_layers
        .values()
        .flatten()
        .filter(|l| l.ends_with("_body") || l.ends_with("_saddle"))
        .cloned()
        .collect();
    let all_asset_types: BTreeSet<&String> = t.asset_layers.values().flatten().collect();
    println!(
        "layer cross-check binding: {} layer type(s) named by the rows; {} body/saddle type(s) of \
         {} declared by {} asset(s)",
        named.len(),
        assets.len(),
        all_asset_types.len(),
        t.asset_layers.len()
    );
    assert_eq!(assets.len(), 15);
    assert_eq!(all_asset_types.len(), 18);
    assert_eq!(named, assets);
    // Every body/saddle slot names its layer, and only those slots do.
    for (id, row) in body_table() {
        for (slot, shown) in &row.slots {
            let animal = slot == "body" || slot == "saddle";
            assert_eq!(
                shown.layer.is_some(),
                animal,
                "{id} `{slot}`: a layer type belongs to a body or saddle slot"
            );
        }
    }
}
