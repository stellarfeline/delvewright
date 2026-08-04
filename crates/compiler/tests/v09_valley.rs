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
