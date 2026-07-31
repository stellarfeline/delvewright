//! Admission tooling: structure round-trip determinism, socket carving, and the
//! static light probe.

use delvewright_admit::fixtures;
use delvewright_admit::light;
use delvewright_admit::meta::{License, PrefabMeta};
use delvewright_admit::socket::{self, SocketDecl};
use delvewright_admit::structure::{Structure, roundtrip};

fn license() -> License {
    License {
        source: "original".into(),
        spdx: "CC0-1.0".into(),
        note: "test".into(),
        provenance: "test".into(),
    }
}

#[test]
fn structure_write_is_deterministic() {
    let s = fixtures::clean_room();
    assert_eq!(s.write(), s.write(), "same structure -> same bytes");
    // round-trip preserves size + palette content.
    let back = roundtrip(&s).unwrap();
    assert_eq!(back.size, s.size);
    assert_eq!(back.block_names(), s.block_names());
    // re-serializing the round-tripped structure is byte-stable too.
    assert_eq!(back.write(), back.write());
}

#[test]
fn socket_carving_places_jigsaw_and_carves_opening() {
    let mut s = fixtures::clean_room(); // 7x5x7, wall at z=0
    let mut meta = PrefabMeta::skeleton("clean", s.size, s.data_version, license());
    // North wall socket at bottom-centre of the opening: x=3, y=1, z=0.
    let decl = SocketDecl::new([3, 1, 0], "north");
    socket::carve(&mut s, &mut meta, &decl).unwrap();

    // jigsaw marker at the socket cell.
    let entry = s.entry_at([3, 1, 0]).unwrap();
    assert_eq!(entry.name, "minecraft:jigsaw");
    assert_eq!(
        entry.properties.get("orientation").map(String::as_str),
        Some("north_up")
    );
    // the block entity is carried.
    assert!(s.block_at([3, 1, 0]).unwrap().nbt.is_some());
    // opening carved to air (a flanking cell of the 3x3, not the jigsaw cell).
    assert_eq!(s.entry_at([2, 1, 0]).unwrap().name, "minecraft:air");
    assert_eq!(s.entry_at([3, 3, 0]).unwrap().name, "minecraft:air");
    // connector recorded once.
    assert_eq!(meta.connectors.len(), 1);
    assert_eq!(meta.connectors[0].facing, "north");
    assert_eq!(meta.connectors[0].local_pos, [3, 1, 0]);

    // idempotent metadata; carving is deterministic.
    let bytes1 = s.write();
    let mut s2 = fixtures::clean_room();
    let mut meta2 = PrefabMeta::skeleton("clean", s2.size, s2.data_version, license());
    socket::carve(&mut s2, &mut meta2, &decl).unwrap();
    assert_eq!(bytes1, s2.write(), "carving is deterministic");

    // the carved piece re-parses.
    let reparsed = Structure::read(&bytes1).unwrap();
    assert_eq!(
        reparsed.entry_at([3, 1, 0]).unwrap().name,
        "minecraft:jigsaw"
    );
}

#[test]
fn socket_out_of_bounds_errors() {
    let mut s = fixtures::clean_room();
    let mut meta = PrefabMeta::skeleton("clean", s.size, s.data_version, license());
    let decl = SocketDecl::new([99, 1, 0], "north");
    assert!(socket::carve(&mut s, &mut meta, &decl).is_err());
}

#[test]
fn light_probe_calls_lit_room_lit_and_dark_room_dark() {
    let lit = light::probe(&fixtures::clean_room(), light::DEFAULT_DARK_THRESHOLD);
    assert_eq!(
        lit.profile, "lit",
        "ceiling glowstone -> lit ({:?})",
        lit.measured_min_light
    );
    assert!(lit.floor_cells > 0);
    assert!(lit.measured_min_light.unwrap() >= light::DEFAULT_DARK_THRESHOLD);

    let dark = light::probe(&fixtures::dark_room(), light::DEFAULT_DARK_THRESHOLD);
    assert_eq!(dark.profile, "dark", "no light source -> dark");
    assert!(dark.is_dark());
    assert_eq!(dark.measured_min_light, Some(0));
}

#[test]
fn light_probe_writes_estimate_method_into_metadata() {
    let s = fixtures::clean_room();
    let probe = light::probe(&s, light::DEFAULT_DARK_THRESHOLD);
    let mut meta = PrefabMeta::skeleton("clean", s.size, s.data_version, license());
    meta.set_lighting_from_probe(&probe);
    assert_eq!(meta.lighting.profile, "lit");
    // honest: the method string marks this as a static estimate, not a live probe.
    assert!(meta.lighting.method.contains("static"));
    assert!(meta.lighting.method.to_lowercase().contains("not a live"));
}
