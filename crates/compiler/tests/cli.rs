//! Process-level CLI tests: exit-code + diagnostic matrix, and the ADR-0006
//! double-build byte-identity determinism gate.

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

#[test]
fn version_line() {
    let out = delvec(&["--version"]);
    assert_eq!(code(&out), 0);
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("delvec 0.1.0"), "{s}");
    assert!(s.contains("dsl 0.3.0"), "{s}");
    assert!(s.contains("mc 1.21.11"), "{s}");
}

#[test]
fn valid_campaign_validates_and_analyzes() {
    let hw = common::hello_world_dir();
    let pf = common::prefabs_dir();
    let v = delvec(&[
        "validate",
        hw.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "validate: {}",
        String::from_utf8_lossy(&v.stdout)
    );
    let a = delvec(&[
        "analyze",
        hw.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&a),
        0,
        "analyze: {}",
        String::from_utf8_lossy(&a.stdout)
    );
}

#[test]
fn build_is_byte_identical_across_runs() {
    let hw = common::hello_world_dir();
    let pf = common::prefabs_dir();
    let out_a = tmp("det-a");
    let out_b = tmp("det-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            hw.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&r), 0, "build: {}", String::from_utf8_lossy(&r.stderr));
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
    // Sanity: the tree actually contains the key contract outputs.
    assert!(a.contains_key("manifest.json"));
    assert!(a.contains_key("critical-path.json"));
    assert!(a.contains_key("datapack/pack.mcmeta"));
    assert!(a.contains_key("server/server.properties"));
}

#[test]
fn invalid_fixtures_exit_1_with_expected_code() {
    let dir = common::dsl_invalid_dir();
    let pf = common::prefabs_dir();
    let mut count = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let patch: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let expect = patch["expect"].as_str().unwrap();
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let camp = tmp(&format!("inv-{stem}"));
        common::materialize(&patch, &camp);

        let out = delvec(&[
            "validate",
            camp.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
            "--json",
        ]);
        let stdout = String::from_utf8_lossy(&out.stdout);

        // DW0143 is registry-dependent: the fixture's "unknown" item
        // (`minecraft:diamond_hoe`) is rejected by the DSL's 5-item vendored
        // subset but is a REAL 1.21.11 item. The compiler injects the full
        // registry, so it correctly accepts it (exit 0) — proving the full
        // registry is wired. See `full_item_registry_accepts_real_items`.
        if expect == "DW0143" {
            assert_eq!(
                code(&out),
                0,
                "DW0143 fixture should pass against the full registry"
            );
            count += 1;
            continue;
        }

        assert_eq!(
            code(&out),
            1,
            "fixture {stem} should exit 1 (validation failure)"
        );
        assert!(
            stdout.contains(expect),
            "fixture {stem}: expected code {expect} in diagnostics:\n{stdout}"
        );
        count += 1;
    }
    assert!(
        count >= 20,
        "expected the full invalid-fixture matrix, saw {count}"
    );
}

#[test]
fn keep_crawl_builds_and_double_build_is_byte_identical() {
    let kc = common::keep_crawl_dir();
    let pf = common::prefabs_dir();
    let out_a = tmp("kc-det-a");
    let out_b = tmp("kc-det-b");
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            kc.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(
            code(&r),
            0,
            "keep-crawl build: {}",
            String::from_utf8_lossy(&r.stderr)
        );
    }
    let a = read_tree(&out_a);
    let b = read_tree(&out_b);
    assert_eq!(a.keys().collect::<Vec<_>>(), b.keys().collect::<Vec<_>>());
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "keep-crawl byte mismatch in {path}");
    }
    // The pool area shipped several distinct structures.
    let structures = a.keys().filter(|p| p.contains("/structure/keep-")).count();
    assert!(
        structures >= 3,
        "multiple keep pieces shipped, saw {structures}"
    );
}

/// The two `DW03xx` build/solver diagnostics: an unsatisfiable required anchor
/// (`DW0302`) and a `pieces` range too small for the required roles (`DW0303`).
/// Both pass validate + analyze (the DSL cannot see pool-area anchors) and fail
/// at build with exit 3 and their exact code in `--json`.
#[test]
fn pool_build_diagnostics_exit_3_with_dw03xx() {
    let pf = common::prefabs_dir();
    for (fixture, expect) in [
        ("keep-unsatisfiable-anchor.json", "DW0302"),
        ("keep-range-too-small.json", "DW0303"),
    ] {
        let patch: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(common::compiler_fixtures_dir().join(fixture)).unwrap(),
        )
        .unwrap();
        let camp = tmp(&format!("pool-{expect}"));
        common::materialize_from(&common::keep_crawl_dir(), &patch, &camp);

        // validate + analyze pass (pool anchors are invisible to the DSL layer).
        let v = delvec(&[
            "analyze",
            camp.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
        ]);
        assert_eq!(code(&v), 0, "{fixture}: analyze should pass");

        let out = tmp(&format!("pool-{expect}-out"));
        let b = delvec(&[
            "build",
            camp.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            pf.to_str().unwrap(),
            "--json",
        ]);
        assert_eq!(code(&b), 3, "{fixture}: build should exit 3");
        let stdout = String::from_utf8_lossy(&b.stdout);
        assert!(
            stdout.contains(expect),
            "{fixture}: expected {expect} in build diagnostics:\n{stdout}"
        );
    }
}

#[test]
fn unreachable_finale_exits_2_with_dw0201() {
    let patch: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(common::compiler_fixtures_dir().join("unreachable-finale.json"))
            .unwrap(),
    )
    .unwrap();
    let pf = common::prefabs_dir();
    let camp = tmp("unreachable-finale");
    common::materialize(&patch, &camp);

    // validate passes (no DSL rule violated) ...
    let v = delvec(&[
        "validate",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(
        code(&v),
        0,
        "validate should pass: {}",
        String::from_utf8_lossy(&v.stdout)
    );

    // ... but analyze rejects it with exit 2 and DW0201.
    let a = delvec(&[
        "analyze",
        camp.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(code(&a), 2, "analyze should exit 2");
    let stdout = String::from_utf8_lossy(&a.stdout);
    assert!(stdout.contains("DW0201"), "expected DW0201:\n{stdout}");

    // build also stops at analysis (exit 2), never emitting.
    let out = tmp("unreachable-out");
    let b = delvec(&[
        "build",
        camp.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        pf.to_str().unwrap(),
    ]);
    assert_eq!(code(&b), 2, "build should exit 2 on unreachable finale");
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
