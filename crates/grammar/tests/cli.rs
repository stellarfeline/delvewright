//! `delve-grammar expand`, driven the way an author drives it.
//!
//! Two properties live here, and neither is reachable from the library.
//!
//! **Order.** `export_prefab` has always refused an unusable id. What shipped
//! was a CLI that refused it *after* printing a complete gate report headed
//! `<id>: pass` and writing `<id>.report.json`, then exited non-zero with no
//! `.nbt`. A verdict a reader stops at, above a failure, is worse than no
//! verdict. So: an input that cannot produce an artifact is refused **before**
//! any work whose output could be mistaken for success, and — the general claim
//! — **no `pass` verdict may precede a non-zero exit**, whatever the cause.
//!
//! **Which export the binary calls.** The library tests prove `export_zone`
//! tiles correctly. They do not prove the binary calls it: `run_expand` used
//! `export_prefab` for as long as the export existed, and a region past the
//! structure-template cap was a refusal *at the CLI* with a suggestion that the
//! author re-author their design as several jigsaw-socketed prefabs. A region
//! an author chose is never the wrong size, so a
//! region past the cap is not an input error at all — it tiles.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvewright_grammar::Program;
use delvewright_grammar::block::BlockState;
use delvewright_grammar::ir::{Material, Node};
use delvewright_grammar::library::store_room;

const GRAMMAR: &str = env!("CARGO_BIN_EXE_delve-grammar");

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dw-grammar-cli-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Write `program` to `<dir>/<filename>` — the filename matters, because it is
/// where the default prefab id comes from.
fn program_file(dir: &Path, filename: &str, program: &Program) -> PathBuf {
    let path = dir.join(filename);
    std::fs::write(&path, serde_json::to_vec_pretty(program).unwrap()).unwrap();
    path
}

fn expand(file: &Path, out: &Path, extra: &[&str]) -> Output {
    expand_over(file, out, "7x5x14", extra)
}

fn expand_over(file: &Path, out: &Path, region: &str, extra: &[&str]) -> Output {
    Command::new(GRAMMAR)
        .args(["expand", "--file"])
        .arg(file)
        .args(["--region", region, "--seed", "3"])
        .args(extra)
        .arg("-o")
        .arg(out)
        .output()
        .unwrap()
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Nothing that looks like a verdict of success may appear.
fn assert_no_pass_verdict(out: &Output) {
    let text = combined(out);
    for line in text.lines() {
        assert!(
            !line.ends_with(": pass"),
            "a `pass` verdict was printed above a non-zero exit:\n{text}"
        );
        assert!(
            !line.contains("  pass  "),
            "a passing gate line was printed above a non-zero exit:\n{text}"
        );
    }
}

fn is_empty_dir(dir: &Path) -> bool {
    !dir.exists() || std::fs::read_dir(dir).unwrap().next().is_none()
}

// ---------------------------------------------------------------------------
// Order: no `pass` above a non-zero exit, and nothing on disk after a refusal
// ---------------------------------------------------------------------------

/// The reproduction: a program in `var-B.json`. The id defaults to the file
/// stem, and `var-B` is not a usable structure id.
#[test]
fn an_unusable_id_is_refused_before_anything_is_expanded_or_written() {
    let dir = scratch("bad-id");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "var-B.json", &store_room());

    let out = expand(&file, &out_dir, &[]);

    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
    assert_no_pass_verdict(&out);

    let text = combined(&out);
    assert!(
        text.contains("not a usable structure id"),
        "the refusal must say what is wrong:\n{text}"
    );
    // ...and where the id came from, which no document the author read mentions.
    assert!(
        text.contains("the stem of the input filename \"var-B.json\""),
        "the refusal must name where the id came from:\n{text}"
    );
    assert!(
        text.contains("--id"),
        "the refusal must name the way out:\n{text}"
    );

    assert!(
        is_empty_dir(&out_dir),
        "a refused expansion must leave no report and no prefab: {:?}",
        std::fs::read_dir(&out_dir).map(|d| d.count())
    );
}

/// The same program under a usable name produces the artifact — so the refusal
/// above is about the id and nothing else.
#[test]
fn the_same_program_under_a_usable_id_writes_the_prefab() {
    let dir = scratch("good-id");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "var-B.json", &store_room());

    let out = expand(&file, &out_dir, &["--id", "var-b"]);

    assert!(out.status.success(), "{}", combined(&out));
    assert!(out_dir.join("var-b.nbt").is_file());
    assert!(out_dir.join("var-b.json").is_file());
    assert!(out_dir.join("var-b.report.json").is_file());
    // The provenance row reaches the file the operator ends up with.
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("var-b.json")).unwrap())
            .unwrap();
    assert_eq!(meta["license"]["generated_by"]["seed"], 3);
    assert!(
        meta["license"]["generated_by"]["program_hash"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
}

/// An `--id` the author supplied by hand is refused the same way, and the
/// refusal names `--id` rather than the filename it did not come from.
#[test]
fn an_explicit_bad_id_names_the_flag_it_came_from() {
    let dir = scratch("bad-flag-id");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "store-room.json", &store_room());

    let out = expand(&file, &out_dir, &["--id", "Store_Room"]);

    assert_eq!(out.status.code(), Some(2), "{}", combined(&out));
    assert_no_pass_verdict(&out);
    let text = combined(&out);
    assert!(text.contains("the id came from --id"), "{text}");
    assert!(is_empty_dir(&out_dir));
}

/// The general claim, on the one failure that is NOT an input property: every
/// gate passes and freezing still refuses, because a structure template may not
/// carry a command block. The refusal must not wear a `pass`.
#[test]
fn a_freeze_refusal_after_green_gates_prints_no_pass_verdict() {
    let dir = scratch("forbidden");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "armed.json", &armed());

    let out = expand(&file, &out_dir, &[]);

    assert_ne!(out.status.code(), Some(0), "{}", combined(&out));
    assert_no_pass_verdict(&out);
    let text = combined(&out);
    assert!(text.contains("command_block"), "{text}");
    assert!(
        text.contains("this refusal is not a gate"),
        "the reader must be told the gates are not what refused:\n{text}"
    );
    assert!(
        !out_dir.join("armed.nbt").exists(),
        "no prefab may be written"
    );
}

/// The same freeze refusal on the **tiled** path, which is the seam between the
/// two properties this file tests: a zone past the cap goes through
/// `export_zone`, which cuts tiles one at a time and can refuse on any of them.
/// A refusal there must still print no `pass` verdict and must still leave the
/// output directory with nothing a later step could pick up — including no
/// partially written tile set.
#[test]
fn a_tiled_freeze_refusal_prints_no_pass_verdict_and_writes_no_tile() {
    let dir = scratch("forbidden-tiled");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "armed.json", &armed());

    let out = expand_over(&file, &out_dir, "60x5x14", &[]);

    assert_ne!(out.status.code(), Some(0), "{}", combined(&out));
    assert_no_pass_verdict(&out);
    let text = combined(&out);
    assert!(text.contains("command_block"), "{text}");
    let written: Vec<String> = std::fs::read_dir(&out_dir)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".nbt") || n == "armed.json")
                .collect()
        })
        .unwrap_or_default();
    assert!(
        written.is_empty(),
        "a refused tiled expansion must leave no tile and no manifest: {written:?}"
    );
}

/// A program whose every cell is a command block: the structure-template strip
/// would silently replace them with air, so freezing refuses after every gate
/// has passed.
///
/// The state is written out in full because the gates it must pass include
/// `states-complete` (`DW0737`): a bare `minecraft:command_block` reds there,
/// and this fixture's whole job is to reach the freeze with every gate green.
fn armed() -> Program {
    Program::new("armed", "all").rule(
        "all",
        Node::Fill {
            material: Material::block(BlockState::with(
                "minecraft:command_block",
                [("conditional", "false"), ("facing", "north")],
            )),
        },
    )
}

// ---------------------------------------------------------------------------
// Packaging: the binary calls `export_zone`, and the cap never reaches an author
// ---------------------------------------------------------------------------

/// A region past the cap on two axes expands, gates and freezes — no refusal,
/// no flag to pass, nothing about 48 in the author's way.
#[test]
fn a_region_past_the_structure_cap_expands_into_a_tile_set() {
    let dir = scratch("oversize");
    let out = Command::new(GRAMMAR)
        .args(["expand", "--program", "castle", "--region", "90x14x130"])
        .args(["--seed", "7", "--id", "grammar-keep", "-o"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "a giant zone is essential content, never a refusal: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // The manifest, and every tile it names, are on disk under the names it
    // gives.
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("grammar-keep.json")).unwrap())
            .unwrap();
    let set = &manifest["structure_set"];
    assert_eq!(set["size"], serde_json::json!([90, 14, 130]));
    assert_eq!(set["grid"], serde_json::json!([2, 1, 3]));
    let parts = set["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 6);
    for part in parts {
        let file = part["file"].as_str().unwrap();
        assert!(dir.join(file).exists(), "{file} was named and not written");
        for axis in 0..3 {
            assert!(
                part["size"][axis].as_i64().unwrap() <= 48,
                "{file} is past the cap on axis {axis}"
            );
        }
    }

    // The manifest carries the same provenance row a single-template prefab
    // does: packaging changed, and nothing else did.
    assert_eq!(manifest["license"]["generated_by"]["seed"], 7);
    assert_eq!(manifest["connectors"], serde_json::json!([]));

    // The gate report is about the zone, and it is still written beside the
    // pieces: packaging changed, judgement did not.
    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("grammar-keep.report.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(report["verdict"], "pass");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("tile(s) in a 2x1x3 grid"),
        "the operator is told the packaging happened: {stderr}"
    );
    assert!(
        stderr.contains("judged the whole zone"),
        "...and that a tile was not what was judged: {stderr}"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// A region that fits still writes the two files it always wrote, under the
/// names it always used. The tiling path must be invisible here.
#[test]
fn a_region_that_fits_writes_one_structure_and_its_metadata() {
    let dir = scratch("fits");
    let out = Command::new(GRAMMAR)
        .args(["expand", "--program", "castle", "--region", "41x14x25"])
        .args(["--seed", "7", "--id", "grammar-castle", "-o"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");

    assert!(dir.join("grammar-castle.nbt").exists());
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("grammar-castle.json")).unwrap())
            .unwrap();
    assert_eq!(meta["structure"]["file"], "grammar-castle.nbt");
    assert!(
        meta.get("structure_set").is_none(),
        "a prefab that fits one template is not a tile set"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

/// **The reachability measurement is bound to the act of expanding, not to a
/// flag and not to a doc line.**
///
/// A gate nothing invokes is not a gate (CLAUDE.md), and an author who never
/// heard of reachability is exactly the author whose upper storey has no stair.
/// So `expand` prints it and writes it into the report on every run, with no
/// flag passed at all — this asserts the binding at the only place it can fail,
/// which is the binary.
#[test]
fn every_expansion_prints_and_records_the_reachability_measurement() {
    let dir = scratch("reachability-always");
    let out_dir = dir.join("out");
    let file = program_file(&dir, "store-room.json", &store_room());

    let out = expand(&file, &out_dir, &[]);
    assert!(out.status.success(), "{}", combined(&out));

    let printed = combined(&out);
    assert!(
        printed.contains("reachability") && printed.contains("grade entry cell(s)"),
        "no reachability line on a run with no flags:\n{printed}"
    );

    let report: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("store-room.report.json")).unwrap(),
    )
    .unwrap();
    let reach = &report["measurements"]["reachability"];
    assert!(
        reach["standable"].as_u64().unwrap() > 0,
        "the measurement bound to nothing: {reach}"
    );
    assert!(reach["entry_cells"].as_u64().unwrap() > 0, "{reach}");
    assert!(reach["reachable"].as_u64().unwrap() > 0, "{reach}");
    // No gate was asked for, so the two opt-in ones must not have appeared —
    // only the always-on spelling/shape/orientation/non-empty ones. The two
    // settling gates are absent for a third reason: this piece holds no stair
    // and no fluid, so they have nothing to judge and report counts instead.
    let ids: Vec<&str> = report["gates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|g| g["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "blocks-exist",
            "shape-complete",
            "states-complete",
            "oriented-fills",
            "non-empty"
        ],
        "{ids:?}"
    );
}
