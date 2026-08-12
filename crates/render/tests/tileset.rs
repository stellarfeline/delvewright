//! A zone that ships as a tile set is reviewed as ONE scene.
//!
//! These go through real gzip-framed structure templates on disk rather than
//! in-memory fixtures, because the thing being claimed is about bytes an
//! exporter wrote and a renderer reads back.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_render::meta::PrefabMeta;
use delvewright_render::occupancy::Clearance;
use delvewright_render::shots;
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
    write_tile_with(dir, part, zone_state);
}

/// Write one tile as a real `.nbt`, taking each zone cell's state from `state`.
fn write_tile_with(dir: &Path, part: &TilePart, state: fn([i32; 3]) -> &'static str) {
    let mut palette: Vec<BlockState> = Vec::new();
    let mut index_of: BTreeMap<String, i32> = BTreeMap::new();
    let mut blocks = Vec::new();
    for x in 0..part.size[0] {
        for y in 0..part.size[1] {
            for z in 0..part.size[2] {
                let state = state([x + part.offset[0], y + part.offset[1], z + part.offset[2]]);
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

/// The zone the eye-shot test stands a body in: a hollow corridor 60 long, cut
/// at z=48. Shell solid, interior open at `x 1..4`, `y 1..3`.
const HOLLOW: [i32; 3] = [6, 5, 60];

fn hollow_state(pos: [i32; 3]) -> &'static str {
    let solid = pos[0] == 0
        || pos[0] == HOLLOW[0] - 1
        || pos[1] == 0
        || pos[1] == HOLLOW[1] - 1
        || pos[2] == 0
        || pos[2] == HOLLOW[2] - 1;
    if solid {
        "minecraft:stone"
    } else {
        "minecraft:air"
    }
}

/// The same two-tile staging as [`stage`], but hollow and carrying one anchor —
/// declared in **zone** coordinates, past the cut, as a real manifest does.
fn stage_hollow(name: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("dw-render-tiles-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let parts = vec![
        TilePart {
            file: "hollow.x0y0z0.nbt".to_string(),
            id: "hollow.x0y0z0".to_string(),
            grid_index: [0, 0, 0],
            offset: [0, 0, 0],
            size: [6, 5, 48],
        },
        TilePart {
            file: "hollow.x0y0z1.nbt".to_string(),
            id: "hollow.x0y0z1".to_string(),
            grid_index: [0, 0, 1],
            offset: [0, 0, 48],
            size: [6, 5, 12],
        },
    ];
    for part in &parts {
        write_tile_with(&dir, part, hollow_state);
    }
    let set = TileSet {
        base: "hollow".to_string(),
        size: HOLLOW,
        part_max: 48,
        grid: [1, 1, 2],
        data_version: DATA_VERSION,
        generator: "crates/grammar".to_string(),
        parts,
    };
    let manifest = dir.join("hollow.json");
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&serde_json::json!({
            "prefab_id": "prefab/hollow",
            "structure_set": set,
            "anchors": { "anchor/far": { "pos": [3, 1, 55], "facing": "north" } },
            "connectors": [],
            "lighting": { "profile": "unmeasured" },
        }))
        .unwrap(),
    )
    .unwrap();
    (dir, manifest)
}

/// The interaction between tiling and the eye-level cameras, which is the one
/// place the two could have silently failed to compose: an eye shot is planned
/// against **blocks**, and on a tiled zone the blocks arrive from several files.
///
/// The load-bearing assertion is the forward clearance. The anchor sits at
/// z=55, in the *second* tile, which is only 12 deep; a planner handed that tile
/// alone could report at most 6 open cells ahead. Reading 53 means the body is
/// standing in the assembled zone and seeing straight through the cut at z=48 —
/// which is what makes the picture the zone's rather than a fragment's.
#[test]
fn an_eye_shot_on_a_tiled_zone_stands_in_the_zone_and_sees_across_the_cut() {
    let (dir, manifest) = stage_hollow("eye");

    let (piece, meta_path) = load_piece(&manifest).unwrap();
    let meta = PrefabMeta::at_path(&meta_path).unwrap().unwrap();
    let plan = shots::plan_piece(piece.structure(), Some(&meta));

    assert_eq!(plan.binding.declared, 1);
    assert_eq!(plan.binding.eligible, 1);
    assert_eq!(
        plan.binding.eye_shots, 1,
        "a tiled zone's anchors must carry eye shots like any other piece's"
    );
    let shot = plan
        .shots
        .iter()
        .find(|s| s.name == "eye-far")
        .expect("the anchor's eye shot");
    let eye = shot.eye.as_ref().unwrap();

    // The anchor's own cell, in ZONE coordinates. A tile-local reading would put
    // the body at z=7 and the picture somewhere else entirely.
    assert_eq!(eye.anchor_cell, [3, 1, 55]);
    assert_eq!(eye.cell, [3, 1, 55]);
    assert!(eye.supported);
    assert_eq!(
        shot.framing,
        shots::Framing::Eye {
            pos: [3.5, 1.0 + delvewright_render::occupancy::EYE_HEIGHT, 55.5]
        }
    );

    // Looking north from z=55 down an open corridor: 54 clear cells (z=54…1),
    // stopped by the shell's end wall at z=0 — 48 of those cells are on the far
    // side of the cut.
    let Clearance::Blocked { open, state } = &eye.clearance else {
        panic!(
            "the corridor's far wall must stop the ray: {:?}",
            eye.clearance
        );
    };
    assert_eq!(*state, "minecraft:stone");
    assert_eq!(
        *open, 54,
        "the eye must see across the z=48 cut; a per-tile view could not exceed 6"
    );

    assert!(
        plan.diagnostics.is_empty(),
        "an anchor standing in open air with a floor owes no DW0727: {:?}",
        plan.diagnostics
    );

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
