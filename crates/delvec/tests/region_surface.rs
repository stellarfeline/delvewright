//! **A region holds a whole site** (spec-0106 §3.1–§3.5, criteria 1–5 and 7).
//!
//! One fixture: a one-box site plan whose place owns its ground
//! (`base: {"owned": 7}`, floor 64, so its claim's bottom is 56) on an `open`
//! fill whose surface stands at 55, and one piece that draws that ground, two
//! rooms on it — the west one lit by its lid, the east one dark — and a lid
//! over both. The place is allocated no seam, so it is entered by the `spawn`
//! and its piece's contract declares zero exterior edges.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

/// The frame: footprint x 1..16, z 1..16 grown by the ring, y 56..72 — seven
/// courses of owned ground, the floor course at 63, eight of headroom and the
/// lid at 72.
const SIZE: [i32; 3] = [18, 17, 18];
/// The piece-local walk plane (world 64).
const WALK: i32 = 8;
/// The east room's x span; the west room is x 1..8 and the wall stands at 9.
const EAST: (i32, i32) = (10, 16);

fn tempdir(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("dw-region-surface-{name}"));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn write_doc(dir: &Path, name: &str, stage: &str, content: serde_json::Value) {
    let doc = serde_json::json!({
        "campaign_id": "blockout", "content": content,
        "dsl_version": delvewright_dsl::DSL_VERSION, "stage": stage,
    });
    std::fs::write(
        dir.join(name),
        delvewright_dsl::to_canonical_string(&doc).unwrap(),
    )
    .unwrap();
}

/// The site plan: `base` and the surface's datum as given, the box's own
/// `lighting` when `lit`.
fn site_plan(base: serde_json::Value, datum: i64, lit: bool, len: i32) -> serde_json::Value {
    let mut b = serde_json::json!({
        "node": "node/house", "min": [1, 1], "extent": [16, len - 2],
        "floor": {"y": 64}, "ceiling": {"clearance": 8}, "base": base
    });
    if lit {
        b["lighting"] = serde_json::json!({"fixture": "torch", "min_light": 3});
    }
    serde_json::json!({
        "region": {"min": [0, 50, 0], "extent": [SIZE[0], 23, len]},
        "boxes": [b],
        "seams": [],
        "datums": [{"id": "datum/ground", "y": datum}],
        "fill": {
            "kind": "open", "surface": "minecraft:grass_block", "below": "minecraft:stone",
            "terrain": {"kind": "flat", "datum": "datum/ground"}
        }
    })
}

/// The campaign: the blockout fixture's cast and words, one place with three
/// stations, one quest binding all three.
fn campaign(dir: &Path, base: serde_json::Value, datum: i64, lit: bool, len: i32) {
    common::copy_dir_all(
        &common::repo_root().join("crates/delvec/tests/fixtures/blockout"),
        dir,
    );
    write_doc(
        dir,
        "layout-graph.json",
        "layout-graph",
        serde_json::json!({
            "nodes": [{
                "id": "node/house",
                "intent": "the whole site",
                "note": "One region: a house standing on the ground it owns.",
                "stations": [
                    {"anchor": "anchor/hearth", "kind": "point"},
                    {"anchor": "anchor/bench", "kind": "point"},
                    {"anchor": "anchor/settle", "kind": "point"}
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
    write_doc(
        dir,
        "site-plan.json",
        "site-plan",
        site_plan(base, datum, lit, len),
    );
    common::patch_file(&dir.join("npcs.json"), |v| {
        v["content"]["npcs"][0]["anchor"] = serde_json::json!("anchor/settle");
    });
    common::patch_file(&dir.join("quests.json"), |v| {
        let q = &mut v["content"]["quests"][0];
        q["cast"]["npc/keeper"]["at"] = serde_json::json!("anchor/settle");
        q["objectives"][1]["anchor"] = serde_json::json!("anchor/hearth");
        q["objectives"][2]["anchor"] = serde_json::json!("anchor/bench");
        let ask = q["on_objective_complete"]["obj/ask"]
            .as_array_mut()
            .unwrap();
        ask.retain(|e| e["type"] != "open-gate");
    });
}

/// The piece's cell, piece-local, for a piece `len` long on z.
fn cell(p: [i32; 3], len: i32) -> &'static str {
    let [x, y, z] = p;
    let ring = x == 0 || x == SIZE[0] - 1 || z == 0 || z == len - 1;
    if y == 0 {
        return "minecraft:stone";
    }
    if y < WALK - 1 {
        return "minecraft:dirt";
    }
    if y == WALK - 1 {
        return if ring {
            "minecraft:grass_block"
        } else {
            "minecraft:polished_andesite"
        };
    }
    if y == SIZE[1] - 1 {
        // The lid: the west room carries its lanterns, the east room none.
        let west = (1..=8).contains(&x);
        return if west && x % 3 == 0 && z % 3 == 0 {
            "minecraft:sea_lantern"
        } else {
            "minecraft:spruce_planks"
        };
    }
    if ring {
        return "minecraft:stone_bricks";
    }
    // The inner wall, with a doorway two high at z 8..9.
    if x == 9 && !((8..=9).contains(&z) && y <= WALK + 1) {
        return "minecraft:stone_bricks";
    }
    "minecraft:air"
}

fn region(from: [i32; 3], to: [i32; 3]) -> serde_json::Value {
    serde_json::json!({"from": from, "to": to})
}

fn piece_meta(exterior: bool, len: i32) -> serde_json::Value {
    let top = SIZE[1] - 2;
    let back = len - 2;
    let mut edges = vec![serde_json::json!(
        {"a": "west", "b": "east", "class": "walk", "rise": 0,
         "via": {"region": "door", "boxes": [region([9, WALK, 8], [9, WALK + 1, 9])]}}
    )];
    if exterior {
        edges.push(serde_json::json!({"a": "west", "b": "exterior", "class": "walk"}));
    }
    serde_json::json!({
        "prefab_id": "prefab/house",
        "structure": {
            "file": "house.nbt", "id": "house", "size": [SIZE[0], SIZE[1], len], "data_version": 4671,
            "generator": "crates/delvec/tests/region_surface.rs"
        },
        "anchors": {
            "seat": {"pos": [4, WALK, 12], "facing": "south", "role": "entry", "resolves_to": "space:west"},
            "hearth": {"pos": [3, WALK, 4], "facing": "south", "resolves_to": "space:west"},
            "settle": {"pos": [6, WALK, 4], "facing": "south", "resolves_to": "space:west"},
            "bench": {"pos": [14, WALK, 4], "facing": "south", "resolves_to": "space:east"}
        },
        "connectors": [],
        "lighting": {"profile": "lit", "measured_min_light": 8, "measured": "2026-10-14"},
        "license": {
            "source": "original", "spdx": "GPL-3.0-or-later", "note": "Test fixture.",
            "provenance": "Written by crates/delvec/tests/region_surface.rs."
        },
        "spatial_contract": {
            "entry": "west",
            "spaces": {
                "west": {"envelope": "enclosed", "boxes": [region([1, WALK, 1], [8, top, back])]},
                "east": {"envelope": "enclosed", "boxes": [region([EAST.0, WALK, 1], [EAST.1, top, back])]}
            },
            "no_body": {},
            "edges": edges,
            "faces": []
        }
    })
}

/// The library: `prefabs/house.{nbt,json}` under `root`. `exterior` adds an
/// `exterior` edge to the contract.
fn piece(root: &Path, exterior: bool) -> PathBuf {
    piece_of(root, exterior, SIZE[2])
}

/// The house `len` long on z: one template within vanilla's 48-per-axis cap,
/// past it a tile set — the `.nbt` tiles `delvewright_dsl::split` cuts and the
/// metadata's `structure_set` naming them, the shape `delvec grammar export`
/// writes for an oversize zone.
fn piece_of(root: &Path, exterior: bool, len: i32) -> PathBuf {
    use delvewright_dsl::split::{TilePart, TileSet, part_filename, plan_split};
    let prefabs = root.join("prefabs");
    std::fs::create_dir_all(&prefabs).unwrap();
    let size = [SIZE[0], SIZE[1], len];
    let plan = plan_split(size, 48);
    let mut meta = piece_meta(exterior, len);
    let mut parts = Vec::new();
    for part in &plan.parts {
        let mut cells: Vec<([i32; 3], &str)> = Vec::new();
        for x in 0..part.size[0] {
            for y in 0..part.size[1] {
                for z in 0..part.size[2] {
                    let at = [x + part.offset[0], y + part.offset[1], z + part.offset[2]];
                    cells.push(([x, y, z], cell(at, len)));
                }
            }
        }
        let file = if plan.is_single() {
            "house.nbt".to_string()
        } else {
            part_filename("house", part.grid_index)
        };
        std::fs::write(
            prefabs.join(&file),
            common::structure_nbt(part.size, &cells),
        )
        .unwrap();
        parts.push(TilePart {
            id: file.trim_end_matches(".nbt").to_string(),
            file,
            grid_index: part.grid_index,
            offset: part.offset,
            size: part.size,
        });
    }
    if !plan.is_single() {
        let set = TileSet {
            base: "house".to_string(),
            size,
            part_max: 48,
            grid: plan.grid,
            data_version: 4671,
            generator: "crates/delvec/tests/region_surface.rs".to_string(),
            parts,
        };
        let m = meta.as_object_mut().unwrap();
        m.remove("structure");
        m.insert("structure_set".into(), serde_json::to_value(set).unwrap());
    }
    std::fs::write(
        prefabs.join("house.json"),
        serde_json::to_string_pretty(&meta).unwrap() + "\n",
    )
    .unwrap();
    prefabs
}

/// The hand-written row `--piece` must reproduce.
fn row() -> serde_json::Value {
    serde_json::json!({
        "place": "node/house", "piece": "prefab/house",
        "anchors": {
            "anchor/node-house": "seat", "spawn": "seat",
            "anchor/hearth": "hearth", "anchor/bench": "bench", "anchor/settle": "settle"
        }
    })
}

fn detail_plan(dir: &Path) {
    write_doc(
        dir,
        "detail-plan.json",
        "detail-plan",
        serde_json::json!({"details": [row()]}),
    );
}

/// A whole fixture under `root/campaigns/blockout`, its library at
/// `root/prefabs` — the layout `delvec prefab audit` reads the place kind from.
fn fixture(tag: &str, base: serde_json::Value, datum: i64, lit: bool) -> (PathBuf, PathBuf) {
    let root = tempdir(tag);
    let dir = root.join("campaigns/blockout");
    std::fs::create_dir_all(&dir).unwrap();
    campaign(&dir, base, datum, lit, SIZE[2]);
    let prefabs = piece(&root, false);
    detail_plan(&dir);
    (dir, prefabs)
}

fn owned() -> serde_json::Value {
    serde_json::json!({"owned": 7})
}

fn run(args: &[&str]) -> (Option<i32>, String, String) {
    let out = Command::new(BIN).args(args).output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn build(dir: &Path, prefabs: &Path, out: &Path) -> (Option<i32>, String) {
    let (code, out, err) = run(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "build",
        dir.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    (code, format!("{out}{err}"))
}

fn validate(dir: &Path, prefabs: &Path) -> (Option<i32>, String) {
    let (code, out, err) = run(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "validate",
        dir.to_str().unwrap(),
    ]);
    (code, format!("{out}{err}"))
}

/// Write a heightmap over the region's 18 x 18 columns: surface `y` per column.
fn heightmap(dir: &Path, h: impl Fn(i64, i64) -> i64) {
    let img = image::GrayImage::from_fn(SIZE[0] as u32, SIZE[2] as u32, |x, z| {
        image::Luma([(h(i64::from(x), i64::from(z)) - 50) as u8])
    });
    let mut bytes = std::io::Cursor::new(Vec::new());
    img.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    std::fs::write(dir.join("terrain.png"), bytes.into_inner()).unwrap();
    common::patch_file(&dir.join("site-plan.json"), |v| {
        v["content"]["fill"]["terrain"] = serde_json::json!({
            "kind": "heightmap", "heightmap": "terrain.png", "base_y": 50, "range": 255
        });
    });
}

// ---------------------------------------------------------------------------
// Criterion 1: the surface
// ---------------------------------------------------------------------------

fn schema(stage: &str) -> serde_json::Value {
    let (code, out, err) = run(&["schema", "--stage", stage]);
    assert_eq!(code, Some(0), "{err}");
    serde_json::from_str(&out).unwrap()
}

#[test]
fn criterion_1_the_site_plan_box_carries_owned_lighting_and_mitigation() {
    let s = serde_json::to_string(&schema("site-plan")).unwrap();
    assert!(
        s.contains("\"owned\""),
        "the `owned` base variant is exported"
    );
    let plan = schema("site-plan");
    let defs = &plan["$defs"];
    let pb = &defs["PlanBox"];
    let props = pb["properties"]
        .as_object()
        .expect("PlanBox has properties");
    for f in ["lighting", "mitigation"] {
        assert!(props.contains_key(f), "PlanBox.{f} is exported");
        let req = pb["required"].as_array().cloned().unwrap_or_default();
        assert!(!req.iter().any(|r| r == f), "PlanBox.{f} is optional");
    }
    assert!(
        serde_json::to_string(&props["lighting"])
            .unwrap()
            .contains("AreaLighting"),
        "boxes[].lighting is the type `areas[].lighting` carried"
    );
    assert!(
        serde_json::to_string(&props["mitigation"])
            .unwrap()
            .contains("AreaMitigation"),
        "boxes[].mitigation is the type `areas[].mitigation` carried"
    );
}

#[test]
fn criterion_1_dsl_version_0_41_0_is_refused() {
    let (dir, prefabs) = fixture("dw0102", owned(), 56, true);
    let world = std::fs::read_to_string(dir.join("world.json")).unwrap();
    let stamped = format!("\"dsl_version\": \"{}\"", delvewright_dsl::DSL_VERSION);
    assert_eq!(world.matches(&stamped).count(), 1);
    std::fs::write(
        dir.join("world.json"),
        world.replace(&stamped, "\"dsl_version\": \"0.41.0\""),
    )
    .unwrap();
    let (code, err) = validate(&dir, &prefabs);
    assert_ne!(code, Some(0));
    assert!(err.contains("DW0102") && err.contains("0.41.0"), "{err}");
}

/// **No area and no pool can be declared** (spec-0106 §3.3): a world stating
/// `areas[]`, with a piece drawn from a `prefab_pool`, is refused at the
/// schema by name. The findings whose class a pool draw or an inter-area
/// crossing carried (a member seated twice, an anchor on a member the draw did
/// not seat, a branch landing in another area) have no object to stand on.
#[test]
fn a_world_declaring_areas_or_a_pool_is_refused_by_name() {
    let (dir, prefabs) = fixture("no-areas", owned(), 56, true);
    let path = dir.join("world.json");
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    world["content"]["areas"] = serde_json::json!([{
        "id": "area/keep", "prefab_pool": "pool/keep", "pieces": 3
    }]);
    std::fs::write(&path, delvewright_dsl::to_canonical_string(&world).unwrap()).unwrap();
    let (code, err) = validate(&dir, &prefabs);
    assert_ne!(code, Some(0));
    assert!(err.contains("DW0100") && err.contains("areas"), "{err}");
}

// ---------------------------------------------------------------------------
// Criterion 2: owned ground
// ---------------------------------------------------------------------------

fn allocation(dir: &Path) -> serde_json::Value {
    let (code, out, err) = run(&["allocation", dir.to_str().unwrap(), "node/house"]);
    assert_eq!(code, Some(0), "{err}");
    serde_json::from_str(&out).unwrap()
}

#[test]
fn criterion_2_an_owned_place_is_handed_its_whole_frame_and_no_fixed_ground() {
    let (dir, prefabs) = fixture("owned", owned(), 56, true);
    let a = allocation(&dir);
    assert_eq!(a["world_min"], serde_json::json!([0, 56, 0]), "lo.y is 56");
    assert_eq!(a["extent"], serde_json::json!(SIZE));
    assert_eq!(
        a["ground"]["fixed"].as_array().unwrap().len(),
        0,
        "zero fixed"
    );
    assert_eq!(a["voids"].as_array().unwrap().len(), 0, "zero voids");
    assert_eq!(a["ground"]["clipped_columns"], 0);
    let (code, err) = validate(&dir, &prefabs);
    assert_eq!(code, Some(0), "{err}");
    assert!(
        err.contains("1 owned-ground place(s), 324 terrain column(s) under them examined, 0 standing over air (DW1021), 0 clipped by the piece."),
        "{err}"
    );

    // The fill writes nothing inside the claim, and its `below` under it.
    let c = common::campaign_at(&dir);
    let mut reads = delvewright_dsl::metrics::Reads::new();
    let (placement, _) = delvec::compiler::blockout::derive(&c, &mut reads).unwrap();
    let mut map = std::collections::BTreeMap::new();
    for f in &placement.mass {
        let lo = [
            i64::from(f.from[0]),
            i64::from(f.from[1]),
            i64::from(f.from[2]),
        ];
        let hi = [i64::from(f.to[0]), i64::from(f.to[1]), i64::from(f.to[2])];
        for cell in delvec::compiler::blockout::cells_of(lo, hi) {
            if f.block == "minecraft:air" {
                map.remove(&cell);
            } else {
                map.insert(cell, f.block.clone());
            }
        }
    }
    let inside = map
        .keys()
        .filter(|c| (0..18).contains(&c[0]) && (0..18).contains(&c[2]) && c[1] >= 56)
        .count();
    assert_eq!(inside, 0, "the fill writes nothing inside the claim");
    assert_eq!(
        map.get(&[5, 55, 5]).map(String::as_str),
        Some("minecraft:grass_block")
    );
    assert_eq!(
        map.get(&[5, 54, 5]).map(String::as_str),
        Some("minecraft:stone")
    );
}

#[test]
fn criterion_2_terrain_standing_higher_is_counted_never_refused() {
    let (dir, prefabs) = fixture("clipped", owned(), 56, true);
    heightmap(&dir, |x, z| {
        if (1..=16).contains(&x) && (1..=16).contains(&z) {
            60
        } else {
            55
        }
    });
    let a = allocation(&dir);
    assert_eq!(
        a["ground"]["clipped_columns"], 256,
        "the footprint's columns"
    );
    let (code, err) = validate(&dir, &prefabs);
    assert_eq!(code, Some(0), "{err}");
    assert!(err.contains("256 clipped by the piece."), "{err}");
}

#[test]
fn criterion_2_one_column_a_course_short_is_dw1021() {
    let (dir, prefabs) = fixture("dw1021", owned(), 56, true);
    heightmap(&dir, |x, z| if (x, z) == (7, 9) { 54 } else { 55 });
    let (code, err) = validate(&dir, &prefabs);
    assert_ne!(code, Some(0));
    assert!(err.contains("DW1021"), "{err}");
    assert!(err.contains("first at [7, 9]"), "{err}");
    assert!(err.contains("surface is y 54, 1 course(s) short"), "{err}");
    assert!(err.contains("in 1 column(s)"), "{err}");
}

#[test]
fn criterion_2_the_same_piece_on_the_ground_writes_the_rings_ground() {
    // Surface at 56 so a ground place's frame is the piece's size (its bottom
    // is the lower of its floor course and the terrain); the piece then writes
    // its own grass and earth where the whole fixes the ring's ground.
    let (dir, prefabs) = fixture("dw0990", serde_json::json!("ground"), 57, true);
    let a = allocation(&dir);
    assert_eq!(a["extent"], serde_json::json!(SIZE));
    let (code, err) = validate(&dir, &prefabs);
    assert_ne!(code, Some(0));
    assert!(err.contains("DW0990"), "{err}");
    // And owned over the same terrain is clean, its 324 columns clipped.
    let (dir, prefabs) = fixture("owned-57", owned(), 57, true);
    let (code, err) = validate(&dir, &prefabs);
    assert_eq!(code, Some(0), "{err}");
    assert!(err.contains("324 clipped by the piece."), "{err}");
}

// ---------------------------------------------------------------------------
// Criterion 3: lighting and mitigation
// ---------------------------------------------------------------------------

#[test]
fn criterion_3_a_declared_place_is_relit_inside_its_piece() {
    let (dir, prefabs) = fixture("lit", owned(), 56, true);
    let out = dir.parent().unwrap().join("out-lit");
    let (code, err) = build(&dir, &prefabs, &out);
    assert_eq!(code, Some(0), "{err}");
    assert!(!err.contains("DW0210"), "{err}");
    let line = err
        .lines()
        .find(|l| l.starts_with("relight binding:"))
        .expect("the relight line is printed");
    assert!(
        line.contains("inside 1 bound piece(s) declaring `boxes[].lighting` (node/house: "),
        "{line}"
    );
    assert!(
        !line.contains("(node/house: 0)"),
        "fixtures were placed: {line}"
    );
}

#[test]
fn criterion_3_an_undeclared_dark_place_is_dw0210_naming_the_piece() {
    let (dir, prefabs) = fixture("dark", owned(), 56, false);
    let (code, err) = build(&dir, &prefabs, &dir.parent().unwrap().join("out-dark"));
    assert_ne!(code, Some(0));
    assert!(err.contains("DW0210"), "{err}");
    assert!(err.contains("bound to `prefab/house`"), "{err}");
    // The plan's own `lighting` still reaches derived interiors only.
    common::patch_file(&dir.join("site-plan.json"), |v| {
        v["content"]["lighting"] = serde_json::json!({"fixture": "torch", "min_light": 3});
    });
    let (code, err) = build(&dir, &prefabs, &dir.parent().unwrap().join("out-dark2"));
    assert_ne!(code, Some(0));
    assert!(
        err.contains("DW0210") && err.contains("bound to `prefab/house`"),
        "{err}"
    );
}

#[test]
fn criterion_3_mitigation_emits_night_vision_over_the_claim() {
    let (dir, prefabs) = fixture("mitigated", owned(), 56, false);
    common::patch_file(&dir.join("site-plan.json"), |v| {
        v["content"]["boxes"][0]["mitigation"] = serde_json::json!("night-vision");
    });
    let out = dir.parent().unwrap().join("out-nv");
    let (code, err) = build(&dir, &prefabs, &out);
    assert_eq!(code, Some(0), "night vision satisfies DW0210: {err}");
    let f = std::fs::read_to_string(
        out.join("datapack/data/blockout/function/night_vision_tick.mcfunction"),
    )
    .unwrap();
    assert!(
        f.contains("@a[x=0,dx=18,y=56,dy=17,z=0,dz=18]"),
        "the selector is the box's claim, x 0..17, y 56..72, z 0..17: {f}"
    );
}

#[test]
fn criterion_3_min_light_15_on_a_box_is_dw0196() {
    let (dir, prefabs) = fixture("dw0196", owned(), 56, true);
    common::patch_file(&dir.join("site-plan.json"), |v| {
        v["content"]["boxes"][0]["lighting"]["min_light"] = serde_json::json!(15);
    });
    let (code, err) = validate(&dir, &prefabs);
    assert_ne!(code, Some(0));
    assert!(
        err.contains("DW0196") && err.contains("/content/boxes/0/lighting/min_light"),
        "{err}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 4: no seam, no way in
// ---------------------------------------------------------------------------

#[test]
fn criterion_4_a_seamless_places_piece_is_audited_green_with_its_zero() {
    let (_dir, prefabs) = fixture("audit", owned(), 56, true);
    let (code, out, err) = run(&[
        "prefab",
        "audit",
        prefabs.join("house.nbt").to_str().unwrap(),
    ]);
    assert_eq!(code, Some(0), "{out}\n{err}");
    let all = format!("{out}{err}");
    assert!(all.contains("allocated no seam"), "{all}");
    assert!(all.contains("0 exterior edge(s)"), "{all}");
}

#[test]
fn criterion_4_the_same_contract_on_a_seamed_place_is_refused() {
    use delvec::grammar::contract::check_kind;
    let root = tempdir("seamed");
    let prefabs = piece(&root, false);
    let meta: delvewright_dsl::prefab::PrefabMeta = delvewright_dsl::prefab::PrefabMeta::from_json(
        &std::fs::read_to_string(prefabs.join("house.json")).unwrap(),
    )
    .unwrap();
    let model = delvec::admit::spatial::library_grid(&prefabs, &meta).unwrap();
    let contract = meta.spatial_contract.as_ref().unwrap();
    let anchors = delvec::admit::spatial::anchor_points(&meta);
    let seamless = check_kind(&model, contract, &anchors, false, true);
    assert!(seamless.is_pass(), "{:?}", seamless.gates);
    let seamed = check_kind(&model, contract, &anchors, false, false);
    let wf = seamed
        .gates
        .iter()
        .find(|g| g.id == "contract-well-formed")
        .unwrap();
    assert!(
        !wf.passed(),
        "the entry owes an exterior edge on a seamed place"
    );
    assert!(wf.detail.contains("claims no way in"), "{}", wf.detail);
}

// ---------------------------------------------------------------------------
// Criterion 5: `--piece`
// ---------------------------------------------------------------------------

fn detail_piece(dir: &Path, prefabs: &Path) -> (Option<i32>, String) {
    let (code, out, err) = run(&[
        "--prefabs",
        prefabs.to_str().unwrap(),
        "detail",
        dir.to_str().unwrap(),
        "node/house",
        "--piece",
        "prefab/house",
    ]);
    (code, format!("{out}{err}"))
}

#[test]
fn criterion_5_piece_writes_the_hand_written_row() {
    let (dir, prefabs) = fixture("piece", owned(), 56, true);
    let hand = std::fs::read(dir.join("detail-plan.json")).unwrap();
    std::fs::remove_file(dir.join("detail-plan.json")).unwrap();
    let (code, err) = detail_piece(&dir, &prefabs);
    assert_eq!(code, Some(0), "{err}");
    assert_eq!(
        std::fs::read(dir.join("detail-plan.json")).unwrap(),
        hand,
        "the row is byte-equal to the hand-written one"
    );
    assert!(
        err.contains(
            "piece binding: 5 of 5 owed name(s) bound; 0 piece anchor(s) of 4 no station names; 3 station(s) declared"
        ),
        "{err}"
    );
}

#[test]
fn criterion_5_a_station_no_anchor_answers_is_dw0842() {
    let (dir, prefabs) = fixture("piece-dw0842", owned(), 56, true);
    std::fs::remove_file(dir.join("detail-plan.json")).unwrap();
    common::patch_file(&dir.join("layout-graph.json"), |v| {
        v["content"]["nodes"][0]["stations"][1]["anchor"] = serde_json::json!("anchor/benches");
    });
    let (code, err) = detail_piece(&dir, &prefabs);
    assert_ne!(code, Some(0));
    assert!(err.contains("DW0842"), "{err}");
    assert!(err.contains("no anchor named `benches`"), "{err}");
    assert!(
        err.contains("`bench`"),
        "the nearest name is offered: {err}"
    );
    assert!(
        !dir.join("detail-plan.json").exists(),
        "nothing was written"
    );
}

#[test]
fn criterion_5_piece_beside_a_program_names_both_doors() {
    let (dir, prefabs) = fixture("piece-program", owned(), 56, true);
    std::fs::create_dir_all(dir.join("programs")).unwrap();
    std::fs::write(dir.join("programs/house.json"), "{}").unwrap();
    let (code, err) = detail_piece(&dir, &prefabs);
    assert_ne!(code, Some(0));
    assert!(
        err.contains("programs/house.json") && err.contains("--piece prefab/house"),
        "{err}"
    );
}

#[test]
fn criterion_4_piece_over_a_seamed_place_requires_the_exterior_edge() {
    // A second place, joined to the house by a seam, makes the house seamed.
    let (dir, prefabs) = fixture("piece-seamed", owned(), 56, true);
    std::fs::remove_file(dir.join("detail-plan.json")).unwrap();
    common::patch_file(&dir.join("layout-graph.json"), |v| {
        let c = &mut v["content"];
        c["nodes"].as_array_mut().unwrap().push(serde_json::json!({
            "id": "node/yard", "intent": "a yard", "note": "The yard beside the house."
        }));
        c["edges"] = serde_json::json!([
            {"id": "edge/house-yard", "a": "node/house", "b": "node/yard", "class": "walk"}
        ]);
    });
    common::patch_file(&dir.join("site-plan.json"), |v| {
        let c = &mut v["content"];
        c["region"]["extent"][0] = serde_json::json!(30);
        c["boxes"].as_array_mut().unwrap().push(serde_json::json!({
            "node": "node/yard", "min": [19, 1], "extent": [8, 8],
            "floor": {"y": 64}, "ceiling": {"open": 4}, "base": {"owned": 7}
        }));
        c["seams"] = serde_json::json!([{
            "edge": "edge/house-yard", "face": "east", "opening": "door",
            "form": "a door out of the house onto the yard"
        }]);
    });
    let (code, err) = detail_piece(&dir, &prefabs);
    assert_ne!(code, Some(0));
    assert!(
        err.contains("does not answer how the place is entered")
            && err.contains("claims no way in"),
        "a seamed place's piece owes the exterior edge its seam meets: {err}"
    );
}

// ---------------------------------------------------------------------------
// Criterion 7: a tile set
// ---------------------------------------------------------------------------

/// The fixture's piece 50 cells long on z — past vanilla's 48 — is a tile set
/// of two, and a whole-site piece bound to its place builds from it: the
/// datapack ships one structure per tile, and `place_verify` holds one
/// sentinel per tile, so a tile that failed to land is a placement that
/// retries rather than a site placed with a slice missing.
#[test]
fn criterion_7_a_whole_site_piece_past_the_cap_builds_as_its_tiles() {
    let root = tempdir("tiled");
    let dir = root.join("campaigns/blockout");
    std::fs::create_dir_all(&dir).unwrap();
    campaign(&dir, owned(), 56, true, 50);
    let prefabs = piece_of(&root, false, 50);
    detail_plan(&dir);
    let names: Vec<String> = std::fs::read_dir(&prefabs)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".nbt"))
        .collect();
    assert_eq!(names.len(), 2, "the piece is a tile set of two: {names:?}");

    let out = root.join("out");
    let (code, err) = build(&dir, &prefabs, &out);
    assert_eq!(code, Some(0), "{err}");

    let structures = out.join("datapack/data/blockout/structure");
    let mut shipped: Vec<String> = std::fs::read_dir(&structures)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("house.") && n.ends_with(".nbt"))
        .collect();
    shipped.sort();
    assert_eq!(
        shipped,
        vec!["house.x0y0z0.nbt", "house.x0y0z1.nbt"],
        "one structure per tile"
    );

    let verify = std::fs::read_to_string(
        out.join("datapack/data/blockout/function/place_verify.mcfunction"),
    )
    .unwrap();
    let sentinels: Vec<[i32; 3]> = verify
        .lines()
        .filter_map(|l| l.strip_prefix("execute if block "))
        .map(|rest| {
            let v: Vec<i32> = rest
                .split_whitespace()
                .take(3)
                .map(|s| s.parse().unwrap())
                .collect();
            [v[0], v[1], v[2]]
        })
        .collect();
    // The piece's claim is x 0..17, y 56..72, z 0..49: a sentinel inside it is
    // one of the house's tiles, and each tile owns the z half it was cut at.
    let in_house: Vec<&[i32; 3]> = sentinels
        .iter()
        .filter(|c| (0..18).contains(&c[0]) && (56..73).contains(&c[1]) && (0..50).contains(&c[2]))
        .collect();
    assert_eq!(in_house.len(), 2, "one sentinel per tile:\n{verify}");
    assert_eq!(
        in_house.iter().filter(|c| c[2] < 48).count(),
        1,
        "one sentinel in each tile's z span:\n{verify}"
    );
}
