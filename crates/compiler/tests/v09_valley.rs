//! spec-0026 W-B slice, process level: the `valley` horizon builds, is
//! byte-identical across a double build (ADR-0006 gate over the surround
//! generator + biome/tile emission), ships its surround tiles through the
//! bootstrap, and `cherry-valley` is proven a **parameter row** of `valley`
//! at the emission level (acceptance criterion 6, `tools/check-flora-parity.py`).

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN).args(args).output().expect("run delvec")
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap_or(-1)
}

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(base, &path, map);
            } else {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                map.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    walk(root, root, &mut map);
    map
}

/// A hello-world clone whose stage-1 world doc declares the given horizon at
/// dsl 0.9.0, with the boundary the non-void bases require (DW0320).
fn valley_campaign(name: &str, horizon: &str) -> std::path::PathBuf {
    let camp = tmp(name);
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let world = format!(
        r#"{{
  "dsl_version": "0.9.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {{
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "{horizon}",
    "boundary": {{ "margin": 20 }},
    "areas": [
      {{ "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }}
    ]
  }}
}}"#
    );
    std::fs::write(camp.join("world.json"), world).unwrap();
    camp
}

fn build(camp: &Path, out: &Path) {
    let pf = common::prefabs_dir();
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&r),
        0,
        "valley build failed:\n{}\n{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
}

/// spec-0026 acceptance criterion 1 (valley row): double-build byte-identity,
/// and the surround actually ships — tiles in the datapack, placement lines in
/// the bootstrap, biome paint in `setup_finish`, the establishing vista in the
/// render plan, and the void ambient in `server.properties`.
#[test]
fn v09_valley_builds_byte_identical_and_ships_surround() {
    let camp = valley_campaign("v09-valley", "valley");
    let out_a = tmp("v09-valley-a");
    let out_b = tmp("v09-valley-b");
    build(&camp, &out_a);
    build(&camp, &out_b);

    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "valley byte mismatch in {path}");
    }

    // The surround shipped as datapack structures…
    let tiles: Vec<_> = a
        .keys()
        .filter(|k| k.contains("structure/horizon/valley/"))
        .collect();
    assert!(!tiles.is_empty(), "no surround tiles in the datapack");
    // …every tile is placed by the bootstrap…
    let place_all =
        String::from_utf8(a["datapack/data/hello-world/function/place_all.mcfunction"].clone())
            .unwrap();
    for t in &tiles {
        let id = t
            .trim_start_matches("datapack/data/hello-world/structure/")
            .trim_end_matches(".nbt");
        assert!(
            place_all.contains(&format!("place template hello-world:{id} ")),
            "tile {id} never placed"
        );
    }
    // …the biome layer paints the annulus through the vanilla-native channel…
    let setup_finish =
        String::from_utf8(a["datapack/data/hello-world/function/setup_finish.mcfunction"].clone())
            .unwrap();
    assert!(
        setup_finish.contains("fillbiome ") && setup_finish.contains("minecraft:windswept_forest"),
        "no fillbiome paint in setup_finish:\n{setup_finish}"
    );
    assert!(
        setup_finish.contains("gamerule max_block_modifications 32768"),
        "the block-modification limit is not restored after the biome paint"
    );
    // …the render plan gains the establishing vista (spec-0026 §6)…
    let render = String::from_utf8(a["render-plan.json"].clone()).unwrap();
    assert!(
        render.contains("\"kind\": \"vista\""),
        "no establishing-vista shot in render-plan.json"
    );
    // …and the ambient below the tile skirt is void (spec-0026 §1).
    let props = String::from_utf8(a["server/server.properties"].clone()).unwrap();
    assert!(
        props.contains("\"biome\":\"minecraft:the_void\""),
        "valley ambient must be the void superflat:\n{props}"
    );
}

/// A private, mutable copy of the prefab library with `prefab/hello-room`'s
/// metadata JSON transformed (the real `campaigns/prefabs` is the content-repo
/// checkout and is never written by a test).
fn doctored_prefabs(name: &str, f: impl Fn(&mut serde_json::Value)) -> std::path::PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let meta_path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
    f(&mut meta);
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

fn build_expecting(camp: &Path, prefabs: &Path, out: &Path, exit: i32, code_str: &str) {
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert_eq!(
        code(&r),
        exit,
        "expected exit {exit} with {code_str}:\n{all}"
    );
    assert!(all.contains(code_str), "expected {code_str}:\n{all}");
}

/// spec-0026 acceptance criterion 2 (`DW0366` by code): a horizon param out of
/// its spec range is a validation error (exit 1).
#[test]
fn v09_out_of_range_ratio_is_dw0366() {
    let camp = tmp("v09-dw0366");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let world = r#"{
  "dsl_version": "0.9.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": { "base": "valley", "ratio": 5.0 },
    "boundary": { "margin": 20 },
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
    ]
  }
}"#;
    std::fs::write(camp.join("world.json"), world).unwrap();
    let out = tmp("v09-dw0366-out");
    build_expecting(&camp, &common::prefabs_dir(), &out, 1, "DW0366");
}

/// spec-0026 acceptance criterion 2 (`DW0367` by code): a piece placed under a
/// non-void horizon whose prefab metadata declares no `walk_y` is a build
/// error — the per-area datum has nothing to compute from.
#[test]
fn v09_missing_walk_y_is_dw0367() {
    let camp = valley_campaign("v09-dw0367", "valley");
    let prefabs = doctored_prefabs("v09-dw0367-prefabs", |meta| {
        meta.as_object_mut().unwrap().remove("walk_y");
    });
    let out = tmp("v09-dw0367-out");
    build_expecting(&camp, &prefabs, &out, 3, "DW0367");
}

/// spec-0026 acceptance criterion 2 (`DW0364` by code) — the #149-shaped
/// fixture: an interior piece whose walk plane lands below sea level and which
/// declares NO `waterline_y` is red. A lying `walk_y` (5, real walk plane
/// local 1) places the base at 63−5=58, landing the standable cells at 59 —
/// four blocks under the ocean; the old `DW0344` exemption would have looked
/// away, the empirical flood proof does not.
#[test]
fn v09_flooded_interior_without_waterline_is_dw0364() {
    let camp = tmp("v09-dw0364");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let world = r#"{
  "dsl_version": "0.9.0",
  "campaign_id": "hello-world",
  "stage": "world",
  "content": {
    "title": "The Keeper's Door",
    "theme": "A lonely keep at the edge of the moor.",
    "premise": "One locked door stands between you and the road home.",
    "seed": 20260729,
    "target_minutes": 5,
    "horizon": "ocean",
    "boundary": { "margin": 20 },
    "areas": [
      { "id": "area/keep", "name": "The Keep", "prefab": "prefab/hello-room" }
    ]
  }
}"#;
    std::fs::write(camp.join("world.json"), world).unwrap();
    let prefabs = doctored_prefabs("v09-dw0364-prefabs", |meta| {
        meta["walk_y"] = serde_json::json!(5);
    });
    let out = tmp("v09-dw0364-out");
    build_expecting(&camp, &prefabs, &out, 3, "DW0364");
}

/// Task #157 finding 2 (the hollow-vigil staging shape): a scene that opens
/// FULLY onto the gap floor — no walls at all — must build green. Before the
/// no-collision-plant fix, phantom tuft-ladders made the annulus perimeter
/// "reachable" and DW0322 fired on hundreds of outer-edge columns; with the
/// collision model fixed, the un-climbable rim (proven by DW0369's flood)
/// bounds the reachable set, so no reachable cell ever borders the void
/// beyond the tiles.
///
/// The fixture is a synthesized 48×48 open floor plate carrying hello-room's
/// anchors (48×48 is the smallest scene whose annulus grows a real rim:
/// side = 36 ≥ gap 12 + slope 18), swapped into a private prefabs copy under
/// the same prefab id.
#[test]
fn v09_open_scene_onto_gap_floor_builds_green() {
    open_scene_builds_green("v09-open", 48, 48, |_, _| true, |_, _| false);
}

/// Task #157 round 4 — the ELEVATED-storey seam. A walls-down scene whose
/// pieces climb (hollow-vigil is a vertical keep: stairs lift the later
/// corridors 4, 8, 12, 16 blocks above the datum) leaves scene-rect columns
/// whose only authored blocks sit ABOVE the gap-floor top. Round 3's moat
/// projected authorship to 2D and read those columns as "the piece owns this
/// ground", so it skipped them — and the gap-floor plane stayed void under the
/// whole elevated footprint, with DW0322 firing along every seam where the
/// filled ground met it (159 columns on the real staging).
///
/// The fixture is the minimal shape: a ground plate with a wing whose blocks
/// exist only on the top storey (local y = 4) and nowhere at the datum. The
/// moat must give that wing ground, exactly as it does for a column no piece
/// mentions at all.
#[test]
fn v09_elevated_storey_over_void_builds_green() {
    open_scene_builds_green(
        "v09-elevated",
        94,
        27,
        |x, z| !(x >= 40 && (8..=18).contains(&z)),
        |x, z| x >= 40 && (8..=18).contains(&z),
    );
}

/// Task #157 round 3 (planner ruling — the void moat): a walls-down scene
/// whose piece authors only an irregular blob INSIDE its declared rect must
/// build green — the scene-rect columns outside the blob receive ambient
/// gap-floor ground, so no DW0322 fires at the rect edge or in the interior.
/// The blob is an L: the rect's other corner region (33×13 columns) is pure
/// moat.
#[test]
fn v09_blob_scene_with_void_moat_builds_green() {
    open_scene_builds_green("v09-blob", 94, 27, |x, z| x <= 60 || z <= 13, |_, _| false);
}

/// The 94×27 twin — hollow-vigil's ACTUAL proportions (task #157 round 2):
/// the short axis's proportional band (20) is under the floor, so it takes
/// the full 30-column band (owner ruling, 2026-08-04: the valley grows to
/// contain any scene shape); the rim profile runs unchanged on every axis
/// and DW0322 stays silent purely by construction.
#[test]
fn v09_hollow_proportioned_open_scene_builds_green() {
    open_scene_builds_green("v09-hollow", 94, 27, |_, _| true, |_, _| false);
}

/// `covered` marks columns carrying the ground plate (local y = 0); `elevated`
/// marks columns carrying ONLY an upper storey (local y = 4) over datum void.
fn open_scene_builds_green(
    name: &str,
    w: i32,
    d: i32,
    covered: impl Fn(i32, i32) -> bool,
    elevated: impl Fn(i32, i32) -> bool,
) {
    let prefabs = tmp(&format!("{name}-prefabs"));
    common::copy_dir_all(&common::prefabs_dir(), &prefabs);

    // --- synthesize the open plate: stone floor at local y=0, iron-bars
    // door region as hello-room authors it, nothing else ---
    #[derive(serde::Serialize, Clone, PartialEq)]
    struct PaletteEntry {
        #[serde(rename = "Name")]
        name: String,
    }
    #[derive(serde::Serialize)]
    struct BlockEntry {
        pos: [i32; 3],
        state: i32,
    }
    #[derive(serde::Serialize)]
    struct Entity {}
    #[derive(serde::Serialize)]
    struct Structure {
        #[serde(rename = "DataVersion")]
        data_version: i32,
        size: [i32; 3],
        palette: Vec<PaletteEntry>,
        blocks: Vec<BlockEntry>,
        entities: Vec<Entity>,
    }
    let mut blocks = Vec::new();
    for x in 0..w {
        for z in 0..d {
            if covered(x, z) {
                blocks.push(BlockEntry {
                    pos: [x, 0, z],
                    state: 0,
                });
            }
            if elevated(x, z) {
                blocks.push(BlockEntry {
                    pos: [x, 4, z],
                    state: 0,
                });
            }
        }
    }
    for x in 4..=5 {
        for y in 1..=3 {
            blocks.push(BlockEntry {
                pos: [x, y, 6],
                state: 1,
            });
        }
    }
    let structure = Structure {
        data_version: 4671,
        size: [w, 5, d],
        palette: vec![
            PaletteEntry {
                name: "minecraft:stone".into(),
            },
            PaletteEntry {
                name: "minecraft:iron_bars".into(),
            },
        ],
        blocks,
        entities: vec![],
    };
    let nbt = fastnbt::to_bytes(&structure).unwrap();
    use std::io::Write as _;
    let mut gz = flate2::GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), flate2::Compression::new(6));
    gz.write_all(&nbt).unwrap();
    std::fs::write(prefabs.join("hello-room.nbt"), gz.finish().unwrap()).unwrap();
    // Patch the metadata: same anchors/walk_y, new size, open-air lighting.
    let meta_path = prefabs.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&meta_path).unwrap()).unwrap();
    meta["structure"]["size"] = serde_json::json!([w, 5, d]);
    std::fs::write(&meta_path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();

    let camp = valley_campaign(&format!("{name}-scene"), "valley");
    // Boundary safety (`DW0322`) is proven by the world-edits pass
    // ([`edit::check_batch_invariants`]) — a campaign with no stage-7 script is
    // never asked the question. The staging these fixtures stand in for IS an
    // edited campaign (hollow-vigil carves its gate yard open), so the fixture
    // carries a one-cell batch: the smallest legal script that puts the whole
    // assembled world, surround and moat included, in front of the proof.
    let edits = serde_json::json!({
        "dsl_version": "0.9.0",
        "campaign_id": "hello-world",
        "stage": "world-edits",
        "content": { "batches": [ {
            "id": "batch/boundary-probe",
            "area": "area/keep",
            "note": "Re-lay one floor cell under the keeper so the batch invariants \
                     (DW0322 boundary safety over the assembled world) run at all.",
            "edits": [
                { "verb": "select", "name": "region/under-keeper", "shape": {
                    "kind": "box",
                    "frame": { "kind": "anchor-relative", "anchor": "anchor/keeper-stand" },
                    "min": [0, -1, 0], "max": [0, -1, 0] } },
                { "verb": "fill", "region": "region/under-keeper", "recipe": {
                    "blocks": [ { "block": "minecraft:stone", "weight": 1.0 } ] } }
            ]
        } ] }
    });
    std::fs::write(
        camp.join("world-edits.json"),
        serde_json::to_string_pretty(&edits).unwrap(),
    )
    .unwrap();
    let out = tmp(&format!("{name}-scene-out"));
    let r = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert_eq!(
        code(&r),
        0,
        "an open scene over the gap floor must build green:\n{all}"
    );
    for dw in ["DW0322", "DW0369"] {
        assert!(
            !all.contains(dw),
            "{dw} must not fire on the staging shape:\n{all}"
        );
    }
}

/// spec-0026 acceptance criterion 6: a same-seed cherry-valley emission
/// differs from the valley emission ONLY in flora/palette block ids and the
/// biome id — script-asserted by `tools/check-flora-parity.py`, the same gate
/// CI runs. (The full flora id map — logs, leaves, understory — is exercised
/// at generator level by `surround::tests`, whose fixture scene is large
/// enough to grow trees; this process-level twin proves the emission plumbing
/// end to end.)
#[test]
fn v09_cherry_valley_is_a_parameter_row() {
    let valley = valley_campaign("v09-parity-valley", "valley");
    let cherry = valley_campaign("v09-parity-cherry", "cherry-valley");
    let out_v = tmp("v09-parity-valley-out");
    let out_c = tmp("v09-parity-cherry-out");
    build(&valley, &out_v);
    build(&cherry, &out_c);

    let script = common::repo_root().join("tools/check-flora-parity.py");
    let r = Command::new("python3")
        .arg(&script)
        .arg(&out_v)
        .arg(&out_c)
        .output()
        .expect("run check-flora-parity.py (python3 required, as for every tools/ gate)");
    assert!(
        r.status.success(),
        "flora parity failed:\n{}\n{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
}
