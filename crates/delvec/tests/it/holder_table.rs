//! spec-0111 criterion 2: every entity holder kind of `delvewright_dsl::holders`
//! is an entity type of the pinned registry — held to the vendored
//! `entities-1.21.11.json`, a document that shares no configuration with the
//! jar reading the table came from — and the admission audit's entity judge
//! admits exactly the table.

#[test]
fn every_holder_kind_is_a_pinned_entity_type() {
    let ids: Vec<String> =
        serde_json::from_str(include_str!("../../data/entities-1.21.11.json")).unwrap();
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
fn the_entity_judge_admits_exactly_the_table() {
    use delvec::schem::nbt::Nbt;
    use std::collections::BTreeMap;
    let entity = |id: &str| {
        let mut nbt = BTreeMap::new();
        nbt.insert("id".to_string(), Nbt::String(id.to_string()));
        let mut c = BTreeMap::new();
        c.insert("nbt".to_string(), Nbt::Compound(nbt));
        Nbt::Compound(c)
    };
    for k in &delvewright_dsl::holders::ENTITY_HOLDERS_1_21_11 {
        let (b, d) = delvec::admit::entities::judge(&[entity(k.id)]);
        assert!(d.is_empty(), "{}: {d:?}", k.id);
        assert_eq!(b.holders, 1, "{}", k.id);
    }
    let (b, d) = delvec::admit::entities::judge(&[entity("minecraft:pig")]);
    assert_eq!((b.not_holders, d[0].code), (1, "DW0730"));
}
