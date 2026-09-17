//! **What a creator runs** (spec-0072 §9): `delvec drawing check` and
//! `delvec drawing execute`, through the binary.
//!
//! The verbs are exercised as an operator exercises them — a document on disk,
//! a region, an output directory — because the thing being checked is the door,
//! and a library call goes round it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const VERSION: &str = delvec::compiler::DSL_VERSION;

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("drawing-cli-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
        .arg("drawing")
        .args(args)
        .output()
        .unwrap()
}

/// A drawing that passes every gate a piece with no contract is held to: a drum,
/// a floor laid only where nothing stands yet, a cone, and a run of treads.
const TOWER: &str = r#"{
  "dsl_version": "@VERSION@",
  "name": "tower",
  "palette": {
    "wall": [ { "weight": 10, "block": "minecraft:stone_bricks" },
              { "weight": 2, "block": "minecraft:cracked_stone_bricks" } ],
    "tread": "minecraft:stone_brick_stairs[facing=north,half=bottom,waterlogged=false]"
  },
  "ops": [
    { "op": "cylinder", "role": "wall", "axis": "y", "t": 1, "from": [0,0,0], "to": [8,7,8],
      "note": "the drum" },
    { "op": "box", "role": "wall", "from": [0,0,0], "to": [8,0,8], "where": ["air"] },
    { "op": "pyramid", "role": "wall", "section": "round", "skin": true,
      "from": [0,8,0], "to": [8,11,8] },
    { "op": "box", "role": "tread", "from": [3,1,7], "to": [5,1,7] }
  ]
}"#;

fn write_tower(dir: &Path) -> PathBuf {
    let path = dir.join("tower.json");
    std::fs::write(&path, TOWER.replace("@VERSION@", VERSION)).unwrap();
    path
}

#[test]
fn check_states_what_it_examined_and_names_a_define_no_use_names() {
    let dir = scratch("check");
    let path = dir.join("hut.json");
    std::fs::write(
        &path,
        format!(
            r#"{{ "dsl_version": "{VERSION}", "name": "hut",
                 "palette": {{ "wall": "minecraft:stone_bricks" }},
                 "defines": {{ "spare": {{ "body": [] }} }},
                 "ops": [ {{ "op": "box", "role": "wall" }} ] }}"#
        ),
    )
    .unwrap();
    let out = delvec(&["check", path.to_str().unwrap()]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("1 operation(s)"), "{stdout}");
    assert!(stdout.contains("1 define(s)"), "{stdout}");
    assert!(stdout.contains("1 role(s)"), "{stdout}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("spare"),
        "a define no `use` names is dead text and is said: {stderr}"
    );
}

/// **The end-to-end verb**: execute, judge, freeze. The report is written
/// whether the gates pass or not, and the piece only when they do.
#[test]
fn execute_writes_the_piece_the_metadata_and_the_report() {
    let dir = scratch("execute");
    let path = write_tower(&dir);
    let out_dir = dir.join("out");
    let out = delvec(&[
        "execute",
        path.to_str().unwrap(),
        "--region",
        "9x12x9",
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    for file in ["tower.nbt", "tower.json", "tower.report.json"] {
        assert!(out_dir.join(file).is_file(), "{file} was not written");
    }
    // The run report, printed on every execution.
    assert!(stderr.contains("operation(s) written"), "{stderr}");
    assert!(stderr.contains("stair(s) settled"), "{stderr}");
    assert!(
        stderr.contains("silent         none"),
        "every written operation did something: {stderr}"
    );
    // The provenance row names the generator, the document and its hash.
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("tower.json")).unwrap())
            .unwrap();
    let by = &meta["license"]["generated_by"];
    assert_eq!(by["generator"], "drawing");
    assert_eq!(by["program"], "tower");
    assert!(by["program_hash"].as_str().unwrap().starts_with("sha256:"));
    assert_eq!(
        meta["structure"]["generator"], "crates/delvec/src/drawing",
        "the breadcrumb names the module that produced the expansion"
    );
}

/// **Determinism, through the door** (spec-0072 criterion 3): the same command
/// twice writes byte-identical files.
#[test]
fn executing_twice_writes_byte_identical_files() {
    let dir = scratch("determinism");
    let path = write_tower(&dir);
    let run = |name: &str| {
        let out_dir = dir.join(name);
        let out = delvec(&[
            "execute",
            path.to_str().unwrap(),
            "--region",
            "9x12x9",
            "-o",
            out_dir.to_str().unwrap(),
        ]);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out_dir
    };
    let a = run("a");
    let b = run("b");
    for file in ["tower.nbt", "tower.json", "tower.report.json"] {
        assert_eq!(
            std::fs::read(a.join(file)).unwrap(),
            std::fs::read(b.join(file)).unwrap(),
            "{file} moved between two runs of one command"
        );
    }
}

/// `--role` is a restyle, and an undeclared one is refused rather than ignored:
/// a typo in a sweep must not build the default silently.
#[test]
fn a_role_override_restyles_and_an_undeclared_one_is_refused() {
    let dir = scratch("roles");
    let path = write_tower(&dir);
    let out_dir = dir.join("out");
    let out = delvec(&[
        "execute",
        path.to_str().unwrap(),
        "--region",
        "9x12x9",
        "--role",
        "wall=minecraft:deepslate_bricks",
        "-o",
        out_dir.to_str().unwrap(),
    ]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta = std::fs::read_to_string(out_dir.join("tower.json")).unwrap();
    assert!(
        meta.contains("wall=minecraft:deepslate_bricks"),
        "the override is in the provenance row: {meta}"
    );

    let out = delvec(&[
        "execute",
        path.to_str().unwrap(),
        "--region",
        "9x12x9",
        "--role",
        "wal=minecraft:deepslate_bricks",
        "-o",
        dir.join("nope").to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("binds no role"), "{stderr}");
    assert!(stderr.contains("\"wall\""), "{stderr}");
}

/// **`delvec prefab diff`** is the instrument a port is judged with: block
/// states cell for cell, the anchors inside the box beside them, and a refusal
/// when it compared nothing.
#[test]
fn prefab_diff_compares_two_pieces_cell_for_cell_and_refuses_an_empty_comparison() {
    let dir = scratch("diff");
    let path = write_tower(&dir);
    let run = |name: &str, extra: &[&str]| {
        let out_dir = dir.join(name);
        let mut args = vec![
            "execute",
            path.to_str().unwrap(),
            "--region",
            "9x12x9",
            "-o",
            out_dir.to_str().unwrap(),
        ];
        args.extend_from_slice(extra);
        let out = delvec(&args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        out_dir.join("tower.nbt")
    };
    let plain = run("plain", &[]);
    let restyled = run("restyled", &["--role", "wall=minecraft:deepslate_bricks"]);

    let diff = |a: &Path, b: &Path, extra: &[&str]| {
        let mut args: Vec<&str> = vec!["prefab", "diff", a.to_str().unwrap(), b.to_str().unwrap()];
        args.extend_from_slice(extra);
        Command::new(env!("CARGO_BIN_EXE_delvec"))
            .args(&args)
            .output()
            .unwrap()
    };

    // A piece against itself: every cell compared, none differing, exit 0.
    let out = diff(&plain, &plain, &[]);
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("972 cell(s) compared, 0 differ"),
        "{stdout}"
    );

    // A restyle moves cells, and the first few are named with BOTH states.
    let out = diff(&plain, &restyled, &[]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("972 cell(s) compared"), "{stdout}");
    assert!(
        !stdout.contains("cell(s) compared, 0 differ"),
        "a restyle moves cells: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("deepslate_bricks"), "{stderr}");
    assert!(stderr.contains("stone_bricks"), "{stderr}");

    // A box outside both pieces compares nothing, and a comparison that
    // examined nothing is a refusal rather than an agreement.
    let out = diff(&plain, &plain, &["--box", "100,100,100,101,101,101"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("ZERO cells compared"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
