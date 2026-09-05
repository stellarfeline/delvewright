//! `delvec grammar expand`, driven the way an author drives it.
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
use delvewright_grammar::library::{ambush_door, store_room};
use delvewright_grammar::{AnchorRole, Mark, MarkAt};

/// `delvec grammar …`: the one binary, entered at the grammar program surface.
fn grammar() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_delvec"));
    cmd.arg("grammar");
    cmd
}

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
    grammar()
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
    let out = grammar()
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
    let out = grammar()
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

// ---------------------------------------------------------------------------
// A `mark`'s role reaches the metadata through the BINARY (§2b)
// ---------------------------------------------------------------------------

/// **The pair this closes.** `grammar.md` §2b says a `mark` carries a `role` and
/// that it is written through to the exported anchor's metadata; §7 used to say
/// a rule could not declare the entry at all. Two authorities for one fact, and
/// the one an author acted on was the wrong one — so this asserts the fact
/// itself, along the path an author walks: a JSON program document, through
/// `delvec grammar expand`, into the metadata file on disk.
///
/// The library test (`tests/marks.rs`) proves the exporter writes it. This
/// proves the BINARY does, which is a different claim: `run_expand` builds the
/// program, applies overrides and calls the exporter itself, and a step that
/// dropped the role on the way through would leave that test green.
#[test]
fn a_marks_role_reaches_the_exported_metadata_through_the_binary() {
    let dir = scratch("mark-role");
    let program = Program::new("arrival", "root").rule(
        "root",
        Node::Mark {
            mark: Mark::new("landing", MarkAt::FloorCenter).role(AnchorRole::Entry),
            body: Box::new(Node::Fill {
                material: Material::block("minecraft:stone_bricks".parse().unwrap()),
            }),
        },
    );
    let file = program_file(&dir, "arrival.json", &program);
    let out_dir = dir.join("out");
    let out = expand(&file, &out_dir, &[]);
    assert!(
        out.status.success(),
        "the marked program expands: {}",
        combined(&out)
    );

    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join("arrival.json")).unwrap())
            .unwrap();
    let anchors = meta["anchors"].as_object().expect("the piece has anchors");
    assert_eq!(
        anchors["anchor/landing"]["role"],
        serde_json::Value::from("entry"),
        "a mark's role is written through to the exported anchor: {meta}"
    );
    // The KEY is untouched, which is the invariant the role exists to work
    // around rather than to break: a mark still cannot name an anchor the DSL
    // could not reference, so no export can ever spell a reserved name.
    for key in anchors.keys() {
        assert!(key.starts_with("anchor/"), "{key}");
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// The provenance record is every input, and a re-expansion from it is the bytes
// ---------------------------------------------------------------------------

/// The `license.generated_by` row, read off a written prefab.
fn record(out_dir: &Path, id: &str) -> serde_json::Value {
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out_dir.join(format!("{id}.json"))).unwrap())
            .unwrap();
    meta["license"]["generated_by"].clone()
}

/// Expand a program document using **only** what a `generated_by` row says, and
/// hand back the `.nbt` bytes. Nothing about the invocation comes from the test.
fn re_expand_from(record: &serde_json::Value, file: &Path, dir: &Path, tag: &str) -> Vec<u8> {
    let region = record["region"]
        .as_array()
        .expect("the record names a region");
    let region = format!("{}x{}x{}", region[0], region[1], region[2]);
    let seed = record["seed"].as_u64().expect("the record names a seed");
    let mut args: Vec<String> = vec![
        "--region".into(),
        region,
        "--seed".into(),
        seed.to_string(),
        "--id".into(),
        tag.into(),
    ];
    for (name, value) in record["params"].as_object().into_iter().flatten() {
        args.push("--param".into());
        args.push(format!("{name}={value}"));
    }
    for (name, value) in record["roles"].as_object().into_iter().flatten() {
        args.push("--role".into());
        args.push(format!("{name}={}", value.as_str().unwrap()));
    }
    let out_dir = dir.join(format!("replay-{tag}"));
    let out = grammar()
        .args(["expand", "--file"])
        .arg(file)
        .args(&args)
        .arg("-o")
        .arg(&out_dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the record's own inputs must expand: {}",
        combined(&out)
    );
    std::fs::read(out_dir.join(format!("{tag}.nbt"))).unwrap()
}

/// **`--param` and `--role` are inputs that reach the bytes, and the record used
/// to omit them.** `generated_by` named the generator, the program, a hash and a
/// seed and claimed those regenerated the NBT byte for byte; the region was in
/// no field at all, and an override left no trace of itself, so re-expanding the
/// named document from the recorded inputs produced a different artifact under a
/// record asserting it would not.
///
/// Three claims. The record carries every input; a re-expansion driven by
/// nothing but the record is byte-identical; and perturbing one `--param` moves
/// the record, moves the hash and moves the bytes — so the record is not merely
/// present but bound to what was built.
#[test]
fn the_provenance_record_is_every_input_and_replaying_it_gives_the_same_bytes() {
    let dir = scratch("provenance");
    let file = program_file(&dir, "ambush-door.json", &ambush_door());
    let region = "11x7x13";

    let expand_with = |tag: &str, extra: &[&str]| -> (serde_json::Value, Vec<u8>) {
        let out_dir = dir.join(tag);
        let out = grammar()
            .args(["expand", "--file"])
            .arg(&file)
            .args(["--region", region, "--seed", "1", "--id", tag])
            .args(extra)
            .arg("-o")
            .arg(&out_dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", combined(&out));
        (
            record(&out_dir, tag),
            std::fs::read(out_dir.join(format!("{tag}.nbt"))).unwrap(),
        )
    };

    // Overridden both ways: an integer knob that changes geometry, and a palette
    // role that changes which blocks are written.
    let (rec, bytes) = expand_with(
        "a",
        &[
            "--param",
            "head=4",
            "--role",
            "stone=minecraft:deepslate_bricks",
        ],
    );
    assert_eq!(rec["region"], serde_json::json!([11, 7, 13]));
    assert_eq!(rec["seed"], serde_json::json!(1));
    assert_eq!(rec["params"], serde_json::json!({ "head": 4 }));
    assert_eq!(
        rec["roles"],
        serde_json::json!({ "stone": "minecraft:deepslate_bricks" })
    );

    // Driven by the record alone.
    assert_eq!(
        re_expand_from(&rec, &file, &dir, "a"),
        bytes,
        "the recorded inputs must regenerate the NBT byte for byte (ADR-0006)"
    );

    // Perturb ONE param. The record moves, the hash moves, the bytes move.
    let (perturbed, perturbed_bytes) = expand_with(
        "b",
        &[
            "--param",
            "head=3",
            "--role",
            "stone=minecraft:deepslate_bricks",
        ],
    );
    assert_eq!(perturbed["params"], serde_json::json!({ "head": 3 }));
    assert_ne!(
        perturbed["program_hash"], rec["program_hash"],
        "an override that changes the program must change the program's hash"
    );
    assert_ne!(
        perturbed_bytes, bytes,
        "the perturbation must be one that reaches the bytes, or this test \
         proves nothing about the record"
    );
    assert_eq!(
        re_expand_from(&perturbed, &file, &dir, "b"),
        perturbed_bytes
    );

    // A program expanded as its document reads writes neither map, so a piece
    // that overrode nothing carries no key saying so.
    let (plain, _) = expand_with("c", &[]);
    assert!(plain.get("params").is_none(), "{plain}");
    assert!(plain.get("roles").is_none(), "{plain}");
    assert_eq!(plain["region"], serde_json::json!([11, 7, 13]));

    let _ = std::fs::remove_dir_all(&dir);
}
