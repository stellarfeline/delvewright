//! The blocks a step fires, derived twice.
//!
//! `delvewright_dsl::stepped_blocks` reads the pinned block registry through
//! `TrapTrigger::is_trigger_block` — an id that ends `_pressure_plate`, or the
//! tripwire string. The pinned block classification derives the plates from a
//! different artifact: Mojang's own `#pressure_plates` block tag
//! (`tools/maintenance/extract-block-classification.py`). A predicate that
//! matched fifteen of sixteen plates, or caught a block the tag does not hold,
//! reds here. Vanilla has no tag for the tripwire string, so it is the one id
//! added by name, and it is checked to be in the registry.

#[test]
fn the_stepped_set_is_the_pressure_plate_tag_and_the_tripwire() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/data/block-classification-1.21.11.json"
    );
    let text = std::fs::read_to_string(path).expect("the pinned block classification");
    let doc: serde_json::Value = serde_json::from_str(&text).expect("classification parses");
    let mut by_tag: Vec<String> = doc["blocks"]
        .as_object()
        .expect("blocks object")
        .iter()
        .filter(|(_, e)| e["form"].as_str() == Some("pressure_plate"))
        .map(|(n, _)| n.clone())
        .collect();
    assert_eq!(by_tag.len(), 16, "the tag binds: {by_tag:#?}");
    assert!(
        delvec::schem::blocks::BlockRegistry::v1_21_11().has("minecraft:tripwire"),
        "the tripwire string is a block of the pin"
    );
    by_tag.push("minecraft:tripwire".to_string());
    by_tag.sort();

    let by_registry: Vec<String> = delvewright_dsl::stepped_blocks()
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(by_registry, by_tag);
}
