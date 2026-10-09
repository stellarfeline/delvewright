//! **Route A is one box** (spec-0098 §5, criterion 6).
//!
//! A site plan with one box and no seam hands its one piece the whole site:
//! walls, lid, ground and all. The places inside the building are the piece's
//! spaces, and the spots the quest layer names are the node's stations, which
//! the piece answers with its own anchors. No switch and no threshold: the
//! campaign below is an ordinary site-plan campaign whose layout graph has one
//! node.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvec::compiler::plan::Plan;
use delvec::compiler::registry::PrefabRegistry;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-one-place-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

/// The frame the one box is handed: footprint x 1..16, z 1..16, floor 64,
/// eight of headroom and a lid, on a `solid` site — so its claim is x 0..17,
/// y 63..72, z 0..17, and the region is exactly that.
const SIZE: [i32; 3] = [18, 10, 18];

/// The three rooms the piece declares, split along x by two inner walls.
const ROOMS: [(&str, i32, i32); 3] = [("west", 1, 5), ("middle", 7, 11), ("east", 13, 16)];

/// The campaign: the blockout fixture's cast and words, one place, one quest.
fn campaign(dir: &Path) {
    common::copy_dir_all(
        &common::repo_root().join("crates/delvec/tests/fixtures/blockout"),
        dir,
    );
    let v = delvewright_dsl::DSL_VERSION;
    let write = |name: &str, stage: &str, content: serde_json::Value| {
        let doc = serde_json::json!({
            "campaign_id": "blockout", "content": content, "dsl_version": v, "stage": stage,
        });
        std::fs::write(
            dir.join(name),
            delvewright_dsl::to_canonical_string(&doc).unwrap(),
        )
        .unwrap();
    };
    write(
        "layout-graph.json",
        "layout-graph",
        serde_json::json!({
            "nodes": [{
                "id": "node/house",
                "intent": "the whole site",
                "note": "One place whose piece carries three rooms.",
                "size_class": "hall",
                "stations": [
                    {"anchor": "anchor/hearth", "kind": "point"},
                    {"anchor": "anchor/bench", "kind": "point"}
                ]
            }],
            "edges": [],
            "entry": "node/house",
            "goal": "node/house",
            "critical_path": ["node/house"],
            "beats": [
                {"node": "node/house", "objective": "obj/ask", "quest": "quest/walk-the-whole"},
                {"node": "node/house", "objective": "obj/enter-the-cell", "quest": "quest/walk-the-whole"},
                {"node": "node/house", "objective": "obj/walk-out", "quest": "quest/walk-the-whole"}
            ]
        }),
    );
    write(
        "site-plan.json",
        "site-plan",
        serde_json::json!({
            "region": {"min": [0, 63, 0], "extent": [SIZE[0], SIZE[1], SIZE[2]]},
            "boxes": [{
                "node": "node/house", "min": [1, 1], "extent": [16, 16],
                "floor": {"y": 64}, "ceiling": {"clearance": 8}
            }],
            "seams": [],
            "fill": {"kind": "solid", "block": "minecraft:deepslate"}
        }),
    );
    common::patch_file(&dir.join("npcs.json"), |v| {
        v["content"]["npcs"][0]["anchor"] = serde_json::json!("anchor/node-house");
    });
    common::patch_file(&dir.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        q["cast"]["npc/keeper"]["at"] = serde_json::json!("anchor/node-house");
        q["objectives"][1]["anchor"] = serde_json::json!("anchor/hearth");
        q["objectives"][2]["anchor"] = serde_json::json!("anchor/bench");
        let ask = q["on_objective_complete"]["obj/ask"]
            .as_array_mut()
            .unwrap();
        ask.retain(|e| e["type"] != "open-gate");
    });
}

/// The piece's cell, piece-local.
fn cell(p: [i32; 3]) -> &'static str {
    let [x, y, z] = p;
    let ring = x == 0 || x == SIZE[0] - 1 || z == 0 || z == SIZE[2] - 1;
    if ring && y == 0 {
        return "minecraft:structure_void"; // the ring's fixed ground
    }
    if y == 0 {
        return "minecraft:polished_andesite";
    }
    if y == SIZE[1] - 1 {
        return if (x % 3 == 0 && z % 3 == 0) || ((x == 6 || x == 12) && (8..=9).contains(&z)) {
            "minecraft:sea_lantern"
        } else {
            "minecraft:spruce_planks"
        };
    }
    if ring {
        return "minecraft:stone_bricks";
    }
    // The two inner walls, each with a doorway two high at z 8..9.
    if (x == 6 || x == 12) && !((8..=9).contains(&z) && y <= 2) {
        return "minecraft:stone_bricks";
    }
    "minecraft:air"
}

fn region(from: [i32; 3], to: [i32; 3]) -> serde_json::Value {
    serde_json::json!({"from": from, "to": to})
}

fn piece(prefabs: &Path) {
    std::fs::create_dir_all(prefabs).unwrap();
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..SIZE[0] {
        for y in 0..SIZE[1] {
            for z in 0..SIZE[2] {
                cells.push(([x, y, z], cell([x, y, z])));
            }
        }
    }
    std::fs::write(
        prefabs.join("house.nbt"),
        common::structure_nbt(SIZE, &cells),
    )
    .unwrap();
    let spaces: serde_json::Map<String, serde_json::Value> = ROOMS
        .iter()
        .map(|(n, x0, x1)| {
            (
                n.to_string(),
                serde_json::json!({"envelope": "enclosed", "boxes": [region([*x0, 1, 1], [*x1, 8, 16])]}),
            )
        })
        .collect();
    let door = |x: i32| region([x, 1, 8], [x, 2, 9]);
    let meta = serde_json::json!({
        "prefab_id": "prefab/house",
        "structure": {
            "file": "house.nbt", "id": "house", "size": SIZE, "data_version": 4671,
            "generator": "crates/delvec/tests/one_place_site.rs"
        },
        "anchors": {
            "seat": {"pos": [9, 1, 4], "facing": "south", "resolves_to": "space:middle"},
            "hearth": {"pos": [3, 1, 4], "facing": "south", "resolves_to": "space:west"},
            "bench": {"pos": [15, 1, 4], "facing": "south", "resolves_to": "space:east"}
        },
        "connectors": [],
        "lighting": {"profile": "lit", "measured_min_light": 8, "measured": "2026-10-09"},
        "license": {
            "source": "original", "spdx": "GPL-3.0-or-later", "note": "Test fixture.",
            "provenance": "Written by crates/delvec/tests/one_place_site.rs."
        },
        "spatial_contract": {
            "entry": "middle",
            "spaces": spaces,
            "no_body": {},
            "edges": [
                {"a": "west", "b": "middle", "class": "walk", "rise": 0,
                 "via": {"region": "west-door", "boxes": [door(6)]}},
                {"a": "middle", "b": "east", "class": "walk", "rise": 0,
                 "via": {"region": "east-door", "boxes": [door(12)]}}
            ],
            "faces": []
        }
    });
    std::fs::write(
        prefabs.join("house.json"),
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .unwrap();
}

fn detail_plan(dir: &Path) {
    let doc = serde_json::json!({
        "campaign_id": "blockout",
        "content": {"details": [{
            "place": "node/house", "piece": "prefab/house",
            "anchors": {
                "anchor/node-house": "seat", "spawn": "seat",
                "anchor/hearth": "hearth", "anchor/bench": "bench"
            }
        }]},
        "dsl_version": delvewright_dsl::DSL_VERSION,
        "stage": "detail-plan",
    });
    std::fs::write(
        dir.join("detail-plan.json"),
        delvewright_dsl::to_canonical_string(&doc).unwrap(),
    )
    .unwrap();
}

/// **Criterion 6.** The one-box, zero-seam campaign validates and builds green;
/// the battery states one place reached over zero seams; the piece is exactly
/// the region; and every cell of the region holds the piece's block — the
/// whole's only where the piece holds `structure_void`, the ring's fixed ground.
#[test]
fn a_one_box_site_is_route_a() {
    let root = tempdir("route-a");
    let (dir, prefabs) = (root.join("campaign"), root.join("prefabs"));
    campaign(&dir);
    piece(&prefabs);
    detail_plan(&dir);

    let c = common::campaign_at(&dir);
    let a = delvec::compiler::detail::allocation(&c, &delvewright_dsl::NodeId("node/house".into()))
        .expect("the one place is allocated");
    let region = c.site_plan.as_ref().unwrap().content.region;
    let extent = [
        i64::from(region.extent[0].get()),
        i64::from(region.extent[1].get()),
        i64::from(region.extent[2].get()),
    ];
    assert_eq!(a.extent, extent, "the piece is the whole region");
    assert_eq!(a.world_min, region.min);

    let out = std::process::Command::new(BIN)
        .args([
            "--prefabs",
            prefabs.to_str().unwrap(),
            "build",
            dir.to_str().unwrap(),
            "--out",
            root.join("out").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(
        err.contains("blockout battery binding: 0 seam(s) proven"),
        "{err}"
    );
    assert!(err.contains("1 place(s) proven reached"), "{err}");
    assert!(
        err.contains("(1 detailed, so 0 massed by the derivation;"),
        "{err}"
    );

    let reg = PrefabRegistry::load_dir(&prefabs).unwrap();
    let plan = Plan::build(&c, &reg).unwrap();
    let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    structures.insert(
        "house.nbt".to_string(),
        std::fs::read(prefabs.join("house.nbt")).unwrap(),
    );
    let blocks = delvec::compiler::assembled::assembled_blocks(&plan, &structures);
    let mut compared = 0usize;
    for x in 0..SIZE[0] {
        for y in 0..SIZE[1] {
            for z in 0..SIZE[2] {
                let world = [x, 63 + y, z];
                let want = match cell([x, y, z]) {
                    "minecraft:structure_void" => Some("minecraft:deepslate"),
                    "minecraft:air" => None,
                    b => Some(b),
                };
                let got = blocks
                    .get(&world)
                    .map(|s| s.to_string())
                    .filter(|s| s != "minecraft:air");
                assert_eq!(got.as_deref(), want, "at {world:?}");
                compared += 1;
            }
        }
    }
    assert_eq!(compared, (SIZE[0] * SIZE[1] * SIZE[2]) as usize);
}

/// **Criterion 10: the model skips `structure_void`.** Over the same campaign,
/// the piece's `structure_void` on the ring's fixed ground leaves the whole's
/// deepslate standing in the assembled model; the same piece with air there
/// removes it, because the game places a template's air.
#[test]
fn the_model_reads_structure_void_as_the_game_does() {
    let root = tempdir("structure-void");
    let (dir, prefabs) = (root.join("campaign"), root.join("prefabs"));
    campaign(&dir);
    piece(&prefabs);
    detail_plan(&dir);
    let c = common::campaign_at(&dir);
    let reg = PrefabRegistry::load_dir(&prefabs).unwrap();
    let plan = Plan::build(&c, &reg).unwrap();
    let at = [0, 63, 0]; // a ring corner's fixed ground, piece-local [0, 0, 0]
    assert_eq!(cell([0, 0, 0]), "minecraft:structure_void");
    let assemble = |nbt: Vec<u8>| {
        let mut structures: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        structures.insert("house.nbt".to_string(), nbt);
        delvec::compiler::assembled::assembled_blocks(&plan, &structures)
    };
    let voided = assemble(std::fs::read(prefabs.join("house.nbt")).unwrap());
    assert_eq!(
        voided.get(&at).map(|s| s.to_string()).as_deref(),
        Some("minecraft:deepslate"),
        "structure_void places nothing; the mass block stands"
    );
    let mut cells: Vec<([i32; 3], &str)> = Vec::new();
    for x in 0..SIZE[0] {
        for y in 0..SIZE[1] {
            for z in 0..SIZE[2] {
                let b = cell([x, y, z]);
                let b = if b == "minecraft:structure_void" {
                    "minecraft:air"
                } else {
                    b
                };
                cells.push(([x, y, z], b));
            }
        }
    }
    let aired = assemble(common::structure_nbt(SIZE, &cells));
    assert!(
        aired
            .get(&at)
            .is_none_or(|s| s.to_string() == "minecraft:air"),
        "air is placed, and carves the mass block: {:?}",
        aired.get(&at)
    );
}
