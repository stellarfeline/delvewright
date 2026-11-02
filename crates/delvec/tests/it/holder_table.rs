//! spec-0111 criterion 2: every entity holder kind of `delvewright_dsl::holders`
//! is an entity type of the pinned registry — held to the vendored
//! `entities-1.21.11.json`, a document that shares no configuration with the
//! jar reading the table came from — and the admission allowlist's default
//! entity list is exactly the table, which a file's `allow_entities` replaces.

#[test]
fn every_holder_kind_is_a_pinned_entity_type() {
    let ids: Vec<String> =
        serde_json::from_str(include_str!("../data/entities-1.21.11.json")).unwrap();
    let table = &delvewright_dsl::holders::ENTITY_HOLDERS_1_21_11;
    assert_eq!(table.len(), 3);
    for k in table {
        assert!(
            ids.iter().any(|i| i == k.id),
            "{} is not in entities-1.21.11.json",
            k.id
        );
    }
}

#[test]
fn the_default_entity_list_is_the_table_and_allow_entities_replaces_it() {
    let default = delvec::admit::allowlist::Allowlist::default();
    for k in &delvewright_dsl::holders::ENTITY_HOLDERS_1_21_11 {
        assert!(default.permits_entity(k.id), "{}", k.id);
    }
    assert!(!default.permits_entity("minecraft:pig"));
    let only_frames = delvec::admit::allowlist::Allowlist::from_file(
        r#"{"allow": ["minecraft:stone"], "allow_entities": ["minecraft:item_frame"]}"#,
    )
    .unwrap();
    assert!(only_frames.permits_entity("minecraft:item_frame"));
    assert!(!only_frames.permits_entity("minecraft:armor_stand"));
    // A file that names no entity list keeps the default.
    let blocks_only =
        delvec::admit::allowlist::Allowlist::from_file(r#"{"allow": ["minecraft:stone"]}"#)
            .unwrap();
    assert!(blocks_only.permits_entity("minecraft:armor_stand"));
}
