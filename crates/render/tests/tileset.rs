//! A zone that ships as a tile set is reviewed as ONE scene.
//!
//! These go through real gzip-framed structure templates on disk rather than
//! in-memory fixtures, because the thing being claimed is about bytes an
//! exporter wrote and a renderer reads back.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_render::tileset::{PieceInput, load_piece};
use delvewright_schem::convert::{self, DATA_VERSION};
use delvewright_schem::schematic::{BlockState, ParsedSchematic};
use delvewright_schem::split::{TilePart, TileSet};

/// The block at a zone cell, in the toy zone these tests tile: a pattern that
/// varies on every axis, so a tile placed at the wrong offset (or with a
/// mis-remapped palette) cannot land on the right answer by accident.
fn zone_state(pos: [i32; 3]) -> &'static str {
    match (pos[0] + 2 * pos[1] + 3 * pos[2]) % 3 {
        0 => "minecraft:stone",
        1 => "minecraft:deepslate",
        _ => "minecraft:cobblestone",
    }
}

/// Write one tile of the toy zone as a real `.nbt`.
fn write_tile(dir: &Path, part: &TilePart) {
    let mut palette: Vec<BlockState> = Vec::new();
    let mut index_of: BTreeMap<String, i32> = BTreeMap::new();
    let mut blocks = Vec::new();
    for x in 0..part.size[0] {
        for y in 0..part.size[1] {
            for z in 0..part.size[2] {
                let state =
                    zone_state([x + part.offset[0], y + part.offset[1], z + part.offset[2]]);
                let index = *index_of.entry(state.to_string()).or_insert_with(|| {
                    palette.push(BlockState {
                        name: state.to_string(),
                        properties: BTreeMap::new(),
                    });
                    palette.len() as i32 - 1
                });
                blocks.push(index);
            }
        }
    }
    let schem = ParsedSchematic {
        version: 3,
        source_data_version: Some(DATA_VERSION),
        size: part.size,
        offset: [0, 0, 0],
        palette,
        blocks,
        block_entities: Vec::new(),
    };
    let mut diagnostics = Vec::new();
    let nbt = convert::build_region(&schem, [0, 0, 0], part.size, &mut diagnostics);
    assert!(diagnostics.is_empty());
    std::fs::write(dir.join(&part.file), nbt).unwrap();
}

/// Lay a two-tile zone down on disk and return its manifest path.
fn stage(name: &str) -> (PathBuf, PathBuf, [i32; 3]) {
    let dir = std::env::temp_dir().join(format!("dw-render-tiles-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let size = [6, 4, 60];
    let parts = vec![
        TilePart {
            file: "zone.x0y0z0.nbt".to_string(),
            id: "zone.x0y0z0".to_string(),
            grid_index: [0, 0, 0],
            offset: [0, 0, 0],
            size: [6, 4, 48],
        },
        TilePart {
            file: "zone.x0y0z1.nbt".to_string(),
            id: "zone.x0y0z1".to_string(),
            grid_index: [0, 0, 1],
            offset: [0, 0, 48],
            size: [6, 4, 12],
        },
    ];
    for part in &parts {
        write_tile(&dir, part);
    }
    let set = TileSet {
        base: "zone".to_string(),
        size,
        part_max: 48,
        grid: [1, 1, 2],
        data_version: DATA_VERSION,
        generator: "crates/grammar".to_string(),
        parts,
    };
    let manifest = dir.join("zone.json");
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&serde_json::json!({
            "prefab_id": "prefab/zone",
            "structure_set": set,
            "anchors": {},
            "lighting": { "profile": "unmeasured" },
        }))
        .unwrap(),
    )
    .unwrap();
    (dir, manifest, size)
}

/// The manifest renders as the whole zone: full extent, every cell where the
/// zone put it. This is what "an author reviews one scene, not N fragments"
/// means mechanically.
#[test]
fn a_manifest_loads_as_the_whole_assembled_zone() {
    let (dir, manifest, size) = stage("whole");

    let (piece, meta_path) = load_piece(&manifest).unwrap();
    let PieceInput::Zone { tiles, grid, .. } = &piece else {
        panic!("a tile-set manifest must load as a zone");
    };
    assert_eq!(*tiles, 2);
    assert_eq!(*grid, [1, 1, 2]);
    assert_eq!(
        meta_path, manifest,
        "the manifest is also where the anchors are read from — a tiled zone's metadata is not \
         beside any one .nbt"
    );

    let st = piece.structure();
    assert_eq!(st.size, size, "the shot plan must frame the ZONE");

    // Every cell, including the ones past the cut, is the block the zone put
    // there. An offset applied to the wrong axis, or a palette index carried
    // across tiles unremapped, fails here.
    let mut seen: BTreeMap<[i32; 3], &str> = BTreeMap::new();
    for (pos, index) in &st.blocks {
        seen.insert(*pos, st.palette[*index].as_str());
    }
    assert_eq!(seen.len() as i32, size[0] * size[1] * size[2]);
    for x in 0..size[0] {
        for y in 0..size[1] {
            for z in 0..size[2] {
                assert_eq!(
                    seen.get(&[x, y, z]).copied(),
                    Some(zone_state([x, y, z])),
                    "cell {x},{y},{z}"
                );
            }
        }
    }
    // ...and in particular the far tile really moved: cells past z=48 exist.
    assert!(seen.keys().any(|p| p[2] >= 48));

    std::fs::remove_dir_all(&dir).unwrap();
}

/// Pointing the renderer at ONE tile is refused, and the refusal names the
/// manifest to use instead. Rendering it would succeed and show a building
/// sliced at a packaging plane — a review that passes and means nothing.
#[test]
fn one_tile_of_a_set_is_refused_and_the_manifest_is_named() {
    let (dir, manifest, _) = stage("fragment");

    let err = load_piece(&dir.join("zone.x0y0z1.nbt")).unwrap_err();
    assert!(err.0.contains("one tile of the zone"), "{}", err.0);
    assert!(
        err.0.contains(manifest.to_str().unwrap()),
        "the refusal must name what to run instead: {}",
        err.0
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// A `.nbt` that no manifest claims still loads exactly as before — the guard
/// keys off membership in a real manifest, never off a filename that happens to
/// have a dot in it.
#[test]
fn an_unclaimed_nbt_still_loads_as_a_single_piece() {
    let (dir, _, _) = stage("unclaimed");
    std::fs::rename(dir.join("zone.json"), dir.join("elsewhere.json")).unwrap();

    let (piece, meta) = load_piece(&dir.join("zone.x0y0z1.nbt")).unwrap();
    assert!(matches!(piece, PieceInput::Single(_)));
    assert_eq!(piece.structure().size, [6, 4, 12]);
    assert_eq!(meta, dir.join("zone.x0y0z1.json"));

    std::fs::remove_dir_all(&dir).unwrap();
}

/// A single-template prefab's metadata is not a manifest, and passing it says
/// so rather than reporting an empty zone.
#[test]
fn a_single_prefabs_metadata_is_not_a_manifest() {
    let dir = std::env::temp_dir().join(format!("dw-render-single-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let meta = dir.join("piece.json");
    std::fs::write(
        &meta,
        r#"{"prefab_id":"prefab/piece","structure":{"file":"piece.nbt","id":"piece"}}"#,
    )
    .unwrap();

    let err = load_piece(&meta).unwrap_err();
    assert!(err.0.contains("not a tile-set manifest"), "{}", err.0);

    std::fs::remove_dir_all(&dir).unwrap();
}
