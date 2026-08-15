//! A zone past the vanilla 48-per-axis cap is placed in world assembly as ONE
//! piece (chunked export phase 2).
//!
//! Phase 1 made an oversize region tile itself at export: the piece ships as
//! several `.nbt` files plus one manifest, and the author never meets the cap.
//! What did not exist was the other end — the compiler answered `DW0346` for a
//! tile-set manifest, so a campaign could *produce* a zone it could not build a
//! world out of.
//!
//! What these tests hold is the property that decides whether that is fixed or
//! merely worked around: **tiling is an export detail, not an authoring
//! concept.** The campaign below binds `prefab/<id>` exactly as it would bind a
//! single-file prefab; its anchors are whole-zone coordinates, one of them past
//! the cut; and the world it gets is the world the piece would have produced if
//! the cap did not exist.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_compiler::emit;
use delvewright_compiler::load::load_campaign_dir;
use delvewright_compiler::plan::Plan;
use delvewright_compiler::registry::PrefabRegistry;
use delvewright_dsl::{Severity, parse_campaign};

/// Where the corridor's anchors sit, in whole-zone coordinates. `exit` is past
/// the cut on purpose: a campaign whose finish line is in the second tile
/// cannot be completed unless the second tile was placed.
fn anchors() -> serde_json::Value {
    serde_json::json!({
        "spawn": { "pos": [4, 1, 2], "facing": "south" },
        "anchor/keeper-stand": { "pos": [4, 1, 4], "facing": "north" },
        "anchor/door": {
            "region": { "from": [4, 1, 30], "to": [4, 3, 30] },
            "block": "minecraft:iron_bars"
        },
        "anchor/exit": { "pos": [4, 1, 55] }
    })
}

/// A scratch dir of this test's own, named for the case.
fn scratch(case: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-tiled-{case}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The hello-world campaign with its one area rebound to `prefab/<id>`.
fn campaign_dir(root: &Path, id: &str) -> PathBuf {
    let dir = root.join("campaign");
    std::fs::create_dir_all(&dir).unwrap();
    for f in common::STAGE_FILES {
        std::fs::copy(common::hello_world_dir().join(f), dir.join(f)).unwrap();
    }
    common::patch_file(&dir.join("world.json"), |v| {
        v["content"]["areas"][0]["prefab"] = serde_json::json!(format!("prefab/{id}"));
    });
    dir
}

/// Build the campaign, reading every structure file the plan names.
fn build(root: &Path, id: &str) -> (Plan<'static>, emit::BuildOutput) {
    let prefabs_dir = root.join("prefabs");
    let campaign = campaign_dir(root, id);
    let loaded = Box::leak(Box::new(load_campaign_dir(&campaign).unwrap()));
    let campaign = Box::leak(Box::new(parse_campaign(&loaded.raw).unwrap()));
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    assert!(
        prefabs
            .load_diagnostics()
            .iter()
            .all(|d| d.severity != Severity::Error),
        "a tile-set manifest must LOAD: {:?}",
        prefabs.load_diagnostics()
    );
    let plan = Plan::build(campaign, &prefabs).expect("plan builds");
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for template in plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .flat_map(|p| &p.templates)
    {
        let bytes = std::fs::read(prefabs_dir.join(&template.structure_file)).unwrap();
        structures.insert(template.structure_file.clone(), bytes);
    }
    let out = emit::build(
        &plan,
        &loaded.inputs,
        &structures,
        &delvewright_compiler::commands::CommandTree::v1_21_11(),
        &prefabs,
        None,
        "test",
        &BTreeMap::new(),
    )
    .expect("build succeeds");
    (plan, out)
}

/// The whole claim, end to end: a campaign binds a tiled zone by its ordinary
/// prefab id, and the datapack places every tile of it at the offset the
/// manifest gives — one logical piece, the size of the whole zone.
#[test]
fn a_tiled_zone_is_bound_like_any_prefab_and_placed_whole() {
    let root = scratch("whole");
    common::write_tiled_zone(&root.join("prefabs"), "tiled-corridor", anchors());
    let (plan, out) = build(&root, "tiled-corridor");

    // ONE logical piece, at the whole zone's size — not two pieces of 48 and 12.
    let pieces: Vec<_> = plan.areas.iter().flat_map(|a| &a.pieces).collect();
    assert_eq!(
        pieces.len(),
        1,
        "a tiled zone is one piece, not one per tile"
    );
    let piece = pieces[0];
    assert_eq!(piece.prefab_id, "prefab/tiled-corridor");
    assert_eq!(piece.size, common::TILED_ZONE_SIZE);
    assert_eq!(piece.templates.len(), 2, "{:?}", piece.templates);

    // The tiles land at the manifest's offsets, in world space.
    assert_eq!(piece.templates[0].pos, piece.pos);
    assert_eq!(
        piece.templates[1].pos,
        [
            piece.pos[0],
            piece.pos[1],
            piece.pos[2] + common::TILED_ZONE_CUT
        ]
    );

    // The forceload AABB spans the WHOLE zone, both tiles.
    let (min, max) = piece.bbox();
    assert_eq!(min, piece.pos);
    assert_eq!(max[2], piece.pos[2] + common::TILED_ZONE_SIZE[2] - 1);

    // Both tiles reach the datapack, and both are placed.
    let place_all =
        String::from_utf8(out["datapack/data/hello-world/function/place_all.mcfunction"].clone())
            .unwrap();
    let lines: Vec<&str> = place_all.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines.len(), 2, "{place_all}");
    assert!(
        lines[0].starts_with("place template hello-world:tiled-corridor.x0y0z0 0 "),
        "{place_all}"
    );
    assert!(
        lines[1].starts_with("place template hello-world:tiled-corridor.x0y0z1 0 "),
        "{place_all}"
    );
    // Same y, and the second tile 48 further along z — the manifest's offset.
    let z: Vec<i32> = lines
        .iter()
        .map(|l| l.split_whitespace().nth(5).unwrap().parse().unwrap())
        .collect();
    assert_eq!(z[1] - z[0], common::TILED_ZONE_CUT);

    for id in ["tiled-corridor.x0y0z0", "tiled-corridor.x0y0z1"] {
        assert!(
            out.contains_key(&format!("datapack/data/hello-world/structure/{id}.nbt")),
            "every tile ships in the datapack; {id} did not"
        );
    }
}

/// The property that makes this a fix rather than a second mechanism: the
/// assembled world is the world the piece would have produced if the cap did
/// not exist.
///
/// Asserted against the zone's own blocks rather than against a count: cells
/// from BOTH tiles are in the model, at their whole-zone coordinates, and the
/// seam is a wall on neither side of the cut.
#[test]
fn the_assembled_world_is_the_zone_the_cut_never_happened_to() {
    let root = scratch("assembled");
    common::write_tiled_zone(&root.join("prefabs"), "tiled-corridor", anchors());
    let prefabs_dir = root.join("prefabs");
    let campaign_path = campaign_dir(&root, "tiled-corridor");
    let loaded = load_campaign_dir(&campaign_path).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&prefabs_dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for template in plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .flat_map(|p| &p.templates)
    {
        let bytes = std::fs::read(prefabs_dir.join(&template.structure_file)).unwrap();
        structures.insert(template.structure_file.clone(), bytes);
    }
    let blocks = delvewright_compiler::assembled::assembled_blocks(&plan, &structures);
    let origin = plan.areas[0].pieces[0].pos;
    let at = |x: i32, y: i32, z: i32| {
        blocks
            .get(&[origin[0] + x, origin[1] + y, origin[2] + z])
            .cloned()
    };

    // A cell from each tile, and the floor running unbroken across the cut.
    assert_eq!(at(0, 0, 0).as_deref(), Some("minecraft:stone"));
    assert_eq!(
        at(4, 0, common::TILED_ZONE_SIZE[2] - 1).as_deref(),
        Some("minecraft:stone"),
        "the far end of the zone is in the world, so the second tile landed"
    );
    for z in common::TILED_ZONE_CUT - 2..common::TILED_ZONE_CUT + 2 {
        assert_eq!(
            at(4, 0, z).as_deref(),
            Some("minecraft:stone"),
            "floor at z={z} — the seam is not a hole"
        );
        assert_eq!(at(4, 2, z), None, "the corridor is open air at z={z}");
    }
    // Nothing of the zone lands past its own extent: a tile placed at the wrong
    // offset would show up here rather than nowhere.
    assert_eq!(at(4, 0, common::TILED_ZONE_SIZE[2]), None);

    // The zone's anchor coordinates are whole-zone coordinates and the cut did
    // not move them: the exit sits past the cut, where the manifest says.
    let exit = plan
        .anchors
        .get(&("area/keep".to_string(), "anchor/exit".to_string()))
        .expect("anchor/exit resolves");
    let delvewright_compiler::plan::ResolvedAnchor::Point { pos, .. } = exit else {
        panic!("anchor/exit is a point anchor");
    };
    assert_eq!(*pos, [origin[0] + 4, origin[1] + 1, origin[2] + 55]);
}

/// A manifest that does not tile its own zone is refused at load, still as
/// `DW0346` — the code keeps meaning "this metadata document is malformed", and
/// stops meaning "this delvec cannot place a tile set".
#[test]
fn a_manifest_that_does_not_tile_its_zone_is_still_dw0346() {
    let root = scratch("holed");
    let dir = root.join("prefabs");
    common::write_tiled_zone(&dir, "tiled-corridor", anchors());
    // Delete the second tile from the manifest, leaving the zone's declared
    // size unchanged: the parts now cover 48 of 60 rows.
    common::patch_file(&dir.join("tiled-corridor.json"), |v| {
        let parts = v["structure_set"]["parts"].as_array_mut().unwrap();
        parts.truncate(1);
    });
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let errors: Vec<_> = prefabs
        .load_diagnostics()
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "DW0346");
    assert!(errors[0].message.contains("cover"), "{:?}", errors[0]);
    assert!(prefabs.get("prefab/tiled-corridor").is_none());
}

/// A tile whose bytes are not the size the manifest declares is refused at
/// build (`DW0803`) rather than placed at an offset that is wrong for it.
///
/// This is the failure tiling makes reachable: the manifest and the tiles are
/// several files, and a stale one slides part of a building through the rest.
/// Every pass but the placement reads the DECLARED size, so nothing else can
/// see it.
#[test]
fn a_tile_that_is_not_the_size_its_manifest_declares_is_dw0803() {
    let root = scratch("stale");
    let dir = root.join("prefabs");
    common::write_tiled_zone(&dir, "tiled-corridor", anchors());

    // The manifest is untouched and still tiles its zone exactly; only the
    // BYTES of the second tile are from another export.
    std::fs::write(
        dir.join("tiled-corridor.x0y0z1.nbt"),
        common::structure_nbt([9, 5, 11], &[([0, 0, 0], "minecraft:stone")]),
    )
    .unwrap();

    let campaign_path = campaign_dir(&root, "tiled-corridor");
    let loaded = load_campaign_dir(&campaign_path).unwrap();
    let campaign = parse_campaign(&loaded.raw).unwrap();
    let prefabs = PrefabRegistry::load_dir(&dir).unwrap();
    let plan = Plan::build(&campaign, &prefabs).unwrap();
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for template in plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .flat_map(|p| &p.templates)
    {
        let bytes = std::fs::read(dir.join(&template.structure_file)).unwrap();
        structures.insert(template.structure_file.clone(), bytes);
    }
    let binding = emit::check_template_extents(&plan, &structures).unwrap_err();
    let emit::BuildFailure::Diagnostic { code, message } = binding else {
        panic!("expected a diagnostic");
    };
    assert_eq!(code, "DW0803");
    assert!(message.contains("tiled-corridor.x0y0z1.nbt"), "{message}");
    assert!(message.contains("9x5x11"), "{message}");
    assert!(message.contains("9x5x12"), "{message}");

    // ...and the honest tiles pass it, with a binding count that is the number
    // of templates actually compared rather than the length of a list.
    common::write_tiled_zone(&dir, "tiled-corridor", anchors());
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for template in plan
        .areas
        .iter()
        .flat_map(|a| a.pieces.iter())
        .flat_map(|p| &p.templates)
    {
        let bytes = std::fs::read(dir.join(&template.structure_file)).unwrap();
        structures.insert(template.structure_file.clone(), bytes);
    }
    let binding = emit::check_template_extents(&plan, &structures).unwrap();
    assert_eq!(binding.placed, 2);
    assert_eq!(binding.checked, 2);
    assert!(binding.finding().is_none());

    // A world whose bytes never arrived binds to nothing, and says so rather
    // than reporting a clean pass over an empty comparison.
    let unbound = emit::check_template_extents(&plan, &BTreeMap::new()).unwrap();
    assert_eq!((unbound.placed, unbound.checked), (2, 0));
    assert_eq!(unbound.finding().unwrap().code, "DW0803");
}

/// Same DSL, same seed, byte-identical datapack — for a tiled placement
/// specifically (ADR-0006).
#[test]
fn a_tiled_placement_is_byte_identical_across_builds() {
    let a = scratch("det-a");
    let b = scratch("det-b");
    common::write_tiled_zone(&a.join("prefabs"), "tiled-corridor", anchors());
    // The SAME input bytes in a second tree, not a second synthesis of them.
    // `fastnbt`'s compound is a `HashMap`, so this file's own generator writes
    // its tags in hash order — running it twice produces two different (equally
    // valid) `.nbt`s. Re-synthesising here would have made the test measure the
    // fixture generator and call the compiler nondeterministic. Two trees, one
    // set of inputs: the claim under test is that the same input builds the same
    // output, and a second tree is what stops it passing on stale state.
    std::fs::create_dir_all(b.join("prefabs")).unwrap();
    for entry in std::fs::read_dir(a.join("prefabs")).unwrap() {
        let path = entry.unwrap().path();
        std::fs::copy(&path, b.join("prefabs").join(path.file_name().unwrap())).unwrap();
    }
    let (_, out_a) = build(&a, "tiled-corridor");
    let (_, out_b) = build(&b, "tiled-corridor");
    assert_eq!(
        out_a.keys().collect::<Vec<_>>(),
        out_b.keys().collect::<Vec<_>>()
    );
    for (path, bytes) in &out_a {
        assert_eq!(bytes, &out_b[path], "{path} differs between builds");
    }
}
