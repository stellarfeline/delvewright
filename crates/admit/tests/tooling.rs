//! Admission tooling: structure round-trip determinism, socket carving, and the
//! static light probe.

use delvewright_admit::fixtures;
use delvewright_admit::jigsaw;
use delvewright_admit::light;
use delvewright_admit::meta::{self, License, PrefabMeta};
use delvewright_admit::socket::{self, SocketDecl};
use delvewright_admit::structure::{Structure, roundtrip};

fn license() -> License {
    License {
        source: "original".into(),
        spdx: "CC0-1.0".into(),
        note: "test".into(),
        provenance: "test".into(),
        generated_by: None,
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
    let mut meta = PrefabMeta::skeleton("clean", s.size, s.data_version, "test", license());
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
    let mut meta2 = PrefabMeta::skeleton("clean", s2.size, s2.data_version, "test", license());
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
fn resolve_jigsaw_replaces_with_final_state_and_prunes() {
    let mut s = fixtures::foreign_jigsaw_piece();
    assert!(s.block_names().contains("minecraft:jigsaw"));

    let resolved = jigsaw::resolve(&mut s);
    assert_eq!(resolved.len(), 3, "three jigsaw markers resolved");

    // plain final_state -> that block.
    assert_eq!(
        s.entry_at([2, 1, 2]).unwrap().name,
        "minecraft:stone_bricks"
    );
    // final_state with block-state properties is parsed.
    let stairs = s.entry_at([4, 1, 4]).unwrap();
    assert_eq!(stairs.name, "minecraft:oak_stairs");
    assert_eq!(
        stairs.properties.get("facing").map(String::as_str),
        Some("east")
    );
    assert_eq!(
        stairs.properties.get("half").map(String::as_str),
        Some("bottom")
    );
    // no final_state -> air fallback.
    assert_eq!(s.entry_at([2, 1, 4]).unwrap().name, "minecraft:air");

    // the jigsaw block entities are gone, and the palette no longer lists jigsaw.
    assert!(s.block_at([2, 1, 2]).unwrap().nbt.is_none());
    assert!(
        !s.block_names().contains("minecraft:jigsaw"),
        "pruned the orphaned jigsaw palette entry"
    );

    // the resolved piece round-trips and is byte-stable.
    let bytes = s.write();
    let back = Structure::read(&bytes).unwrap();
    assert_eq!(back.block_names(), s.block_names());
    assert_eq!(bytes, back.write());

    // idempotent: a second resolve is a no-op.
    let again = jigsaw::resolve(&mut s);
    assert!(again.is_empty());
}

#[test]
fn socket_out_of_bounds_errors() {
    let mut s = fixtures::clean_room();
    let mut meta = PrefabMeta::skeleton("clean", s.size, s.data_version, "test", license());
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
    let mut meta = PrefabMeta::skeleton("clean", s.size, s.data_version, "test", license());
    meta::set_lighting_from_probe(&mut meta, &probe);
    assert_eq!(meta.lighting.profile, "lit");
    // honest: the method string marks this as a static estimate, not a live probe.
    assert!(meta.lighting.method.as_deref().unwrap().contains("static"));
    assert!(
        meta.lighting
            .method
            .as_deref()
            .unwrap()
            .to_lowercase()
            .contains("not a live")
    );
}

/// **`delve-admit` must be able to read what the generators write.**
///
/// The grammar back end (spec-0027 §2) exports `{"profile": "unmeasured"}` and
/// no measurement, because a piece that has not been probed carries no probe
/// result — which is exactly what `delvewright_dsl::registry::Lighting` demands
/// and what the compiler's `PrefabRegistry` accepts. This crate's own metadata
/// copy required all four lighting fields, so every admission subcommand
/// (`socket`, `anchor`, `lighting`, `catalog`) refused a grammar-exported prefab
/// at `DW0732` before it did anything: the admission half of the pipeline was
/// closed to the generated half, and nothing said so.
#[test]
fn an_unmeasured_lighting_block_parses_the_way_the_generators_write_it() {
    let json = r#"{
      "prefab_id": "prefab/arcade-undercroft",
      "structure": {
        "file": "arcade-undercroft.nbt",
        "id": "arcade-undercroft",
        "size": [9, 6, 21],
        "data_version": 4671,
        "generator": "crates/grammar"
      },
      "anchors": { "anchor/undercroft-floor": { "pos": [4, 1, 10], "facing": "north" } },
      "lighting": { "profile": "unmeasured" },
      "license": {
        "source": "original",
        "spdx": "GPL-3.0-or-later",
        "note": "n",
        "provenance": "p"
      }
    }"#;
    let meta = PrefabMeta::from_json(json).expect("a grammar prefab's metadata must parse");
    assert_eq!(meta.lighting.profile, "unmeasured");
    assert_eq!(meta.lighting.measured_min_light, None);
    assert_eq!(meta.lighting.measured, None);
    assert!(meta.connectors.is_empty(), "the export emits no connectors");
}

/// ...and a probe still writes a measurement, because the DSL refuses a
/// `lit`/`dim`/`dark` profile that does not carry one. The optional fields are a
/// widening of what can be READ, never of what a measured claim must say.
#[test]
fn a_probed_profile_still_carries_its_measurement() {
    let mut meta = PrefabMeta::skeleton(
        "probed",
        [5, 5, 5],
        4671,
        "test",
        License {
            source: "original".into(),
            spdx: "GPL-3.0-or-later".into(),
            note: "n".into(),
            provenance: "p".into(),
            generated_by: None,
        },
    );
    assert_eq!(
        meta.lighting.profile, "unmeasured",
        "a skeleton has not been probed and must say so in a profile the DSL has"
    );
    let probe = light::probe(&fixtures::dark_room(), light::DEFAULT_DARK_THRESHOLD);
    meta::set_lighting_from_probe(&mut meta, &probe);
    assert!(meta.lighting.measured_min_light.is_some());
    assert!(meta.lighting.measured.is_some());
}
