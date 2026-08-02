//! The map editor's stage-7 edit stage (spec-0017): the ADR-0006 determinism
//! gate extended over the edit replay, the runtime materialization contract,
//! the per-batch invariant re-proofs (`DW0322` boundary safety, `DW0323`
//! frame/region resolution), and the `delvec edit apply|preview` loop.
//!
//! Process-level (the real `delvec` binary), like `tests/cli.rs`.

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_delvec");

fn delvec(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(common::repo_root())
        .output()
        .expect("run delvec")
}

fn tmp(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn read_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .expect("relative")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, std::fs::read(&path).expect("read file"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn edits_fixture_dir() -> PathBuf {
    common::compiler_fixtures_dir().join("v06-edits")
}

fn prefabs_arg() -> String {
    common::prefabs_dir().display().to_string()
}

/// A private mutable copy of the v06-edits fixture (whole dir, incl. the
/// stage-7 script — `materialize_from` copies only the six base stages).
fn edits_copy(name: &str) -> PathBuf {
    let dst = tmp(name);
    common::copy_dir_all(&edits_fixture_dir(), &dst);
    dst
}

/// Overwrite the copy's `world-edits.json` content with the given batches.
fn set_batches(dir: &Path, batches: serde_json::Value) {
    let doc = serde_json::json!({
        "dsl_version": "0.6.0",
        "campaign_id": "hello-world",
        "stage": "world-edits",
        "content": { "batches": batches }
    });
    std::fs::write(
        dir.join("world-edits.json"),
        serde_json::to_string_pretty(&doc).unwrap(),
    )
    .unwrap();
}

/// ADR-0006 over the edit stage (spec-0017 invariant 3): building the edited
/// fixture twice is byte-identical — the replay derives every noise sample from
/// the campaign seed + script position, nothing else.
#[test]
fn v06_edits_double_build_is_byte_identical() {
    let dir = edits_fixture_dir();
    let (out_a, out_b) = (tmp("edits-det-a"), tmp("edits-det-b"));
    for out in [&out_a, &out_b] {
        let r = delvec(&[
            "build",
            dir.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
            "--prefabs",
            &prefabs_arg(),
        ]);
        assert!(
            r.status.success(),
            "edited build failed:\n{}{}",
            String::from_utf8_lossy(&r.stdout),
            String::from_utf8_lossy(&r.stderr)
        );
    }
    let (a, b) = (read_tree(&out_a), read_tree(&out_b));
    assert_eq!(
        a.keys().collect::<Vec<_>>(),
        b.keys().collect::<Vec<_>>(),
        "same file set"
    );
    for (path, bytes) in &a {
        assert_eq!(bytes, &b[path], "byte mismatch in {path}");
    }
}

/// The runtime materialization contract: the edited build emits a
/// `world_edits` function (coalesced `fill`/`setblock` lines only), calls it
/// from `setup_finish` BEFORE the relight fixtures, and hashes the stage-7
/// script into the manifest inputs.
#[test]
fn edited_build_emits_world_edits_function_and_hashes_the_script() {
    let dir = edits_fixture_dir();
    let out = tmp("edits-emit");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(r.status.success(), "build failed");

    let body = std::fs::read_to_string(
        out.join("datapack/data/hello-world/function/world_edits.mcfunction"),
    )
    .expect("world_edits.mcfunction emitted");
    assert!(!body.trim().is_empty(), "has commands");
    for line in body.lines() {
        assert!(
            line.starts_with("fill ") || line.starts_with("setblock "),
            "world_edits holds only block writes, got: {line}"
        );
    }

    let setup_finish = std::fs::read_to_string(
        out.join("datapack/data/hello-world/function/setup_finish.mcfunction"),
    )
    .expect("setup_finish emitted");
    assert!(
        setup_finish.contains("function hello-world:world_edits"),
        "setup_finish calls world_edits"
    );

    let manifest = std::fs::read_to_string(out.join("manifest.json")).expect("manifest");
    assert!(
        manifest.contains("world-edits.json"),
        "the stage-7 script is a hashed build input (ADR-0006 provenance)"
    );
}

/// An unedited campaign stays on the pre-stage-7 path: no `world_edits`
/// function, no call from `setup_finish` (byte-identity for every existing
/// campaign is separately proven by the pre-existing double-build gates).
#[test]
fn unedited_campaign_emits_no_world_edits_function() {
    let out = tmp("edits-none");
    let r = delvec(&[
        "build",
        common::hello_world_dir().to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(r.status.success(), "hello-world build failed");
    assert!(
        !out.join("datapack/data/hello-world/function/world_edits.mcfunction")
            .exists(),
        "no edit script → no world_edits function"
    );
    let setup_finish = std::fs::read_to_string(
        out.join("datapack/data/hello-world/function/setup_finish.mcfunction"),
    )
    .expect("setup_finish emitted");
    assert!(!setup_finish.contains("world_edits"));
}

/// Boundary safety (spec-0017 invariant 4, `DW0322`): carving a walkable
/// breach through the room's outer wall exposes reachable floor to the void —
/// the build fails naming the batch, never ships the hazard.
#[test]
fn edit_breaching_the_outer_wall_is_dw0322() {
    let dir = edits_copy("edits-breach");
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/breach-wall",
            "area": "area/keep",
            "edits": [
                { "verb": "select", "name": "region/breach", "shape": {
                    "kind": "box",
                    "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                    "min": [4, 1, 0], "max": [6, 2, 0]
                }},
                { "verb": "carve", "region": "region/breach" }
            ]
        }]),
    );
    let out = tmp("edits-breach-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert_eq!(r.status.code(), Some(3), "build-tier failure");
    let stdout = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(stdout.contains("DW0322"), "expected DW0322:\n{stdout}");
    assert!(
        stdout.contains("batch/breach-wall"),
        "names the batch:\n{stdout}"
    );
}

/// Frame drift (`DW0323`): a piece-local frame whose declared prefab no longer
/// matches the solved layout is a loud resolution error, never a silently
/// misplaced edit.
#[test]
fn edit_frame_prefab_drift_is_dw0323() {
    let dir = edits_copy("edits-drift");
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/drift",
            "area": "area/keep",
            "edits": [
                { "verb": "select", "name": "region/x", "shape": {
                    "kind": "box",
                    "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/keep-hall" },
                    "min": [1, 0, 1], "max": [2, 0, 2]
                }},
                { "verb": "carve", "region": "region/x" }
            ]
        }]),
    );
    let out = tmp("edits-drift-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert_eq!(r.status.code(), Some(3));
    let stdout = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(stdout.contains("DW0323"), "expected DW0323:\n{stdout}");
}

/// A verb whose region resolves to zero cells is a silent no-op — always a
/// defect (`DW0323`), never something to ship quietly.
#[test]
fn edit_with_empty_region_is_dw0323() {
    let dir = edits_copy("edits-empty");
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/empty",
            "area": "area/keep",
            "edits": [
                { "verb": "select", "name": "region/box", "shape": {
                    "kind": "box",
                    "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                    "min": [1, 2, 1], "max": [3, 3, 3]
                }},
                // The box is interior air — no diorite matches anywhere.
                { "verb": "select", "name": "region/none", "shape": {
                    "kind": "palette-match", "within": "region/box",
                    "blocks": ["minecraft:diorite"]
                }},
                { "verb": "carve", "region": "region/none" }
            ]
        }]),
    );
    let out = tmp("edits-empty-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert_eq!(r.status.code(), Some(3));
    let stdout = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(stdout.contains("DW0323"), "expected DW0323:\n{stdout}");
    assert!(stdout.contains("zero cells"), "names the defect:\n{stdout}");
}

/// The `edit preview` / `edit apply` loop contract: preview renders a green
/// candidate batch without touching the campaign dir; apply persists it into
/// `world-edits.json` (canonical form) and the persisted script then replays
/// standalone. Both write one labelled snapshot + manifest per batch.
#[test]
fn edit_preview_never_persists_and_apply_persists_canonically() {
    let dir = edits_copy("edits-loop");
    let before = std::fs::read_to_string(dir.join("world-edits.json")).unwrap();
    let batch_file = tmp("edits-loop-batch").with_extension("json");
    std::fs::write(
        &batch_file,
        serde_json::to_string_pretty(&serde_json::json!({
            "id": "batch/keeper-pad",
            "area": "area/keep",
            "edits": [
                { "verb": "select", "name": "region/pad", "shape": {
                    "kind": "box",
                    "frame": { "kind": "anchor-relative", "anchor": "anchor/keeper-stand" },
                    "min": [-1, -1, -1], "max": [1, -1, 1]
                }},
                { "verb": "fill", "region": "region/pad", "recipe": {
                    "blocks": [
                        { "block": "minecraft:polished_diorite", "weight": 1.0 },
                        { "block": "minecraft:diorite", "weight": 1.0 }
                    ]
                }}
            ]
        }))
        .unwrap(),
    )
    .unwrap();

    // Preview: green, renders, persists nothing.
    let shots = tmp("edits-loop-shots");
    let r = delvec(&[
        "edit",
        "preview",
        dir.to_str().unwrap(),
        "--batch",
        batch_file.to_str().unwrap(),
        "-o",
        shots.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(
        r.status.success(),
        "preview failed:\n{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("world-edits.json")).unwrap(),
        before,
        "preview never writes the script"
    );
    assert!(shots.join("keeper-pad.png").exists(), "candidate rendered");
    assert!(
        shots.join("dress-floor.png").exists(),
        "existing batches rendered"
    );
    assert!(shots.join("dress-floor.manifest.json").exists());

    // Apply: same batch persists, canonically parseable, and replays standalone.
    let r = delvec(&[
        "edit",
        "apply",
        dir.to_str().unwrap(),
        "--batch",
        batch_file.to_str().unwrap(),
        "-o",
        shots.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(
        r.status.success(),
        "apply failed:\n{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    let after = std::fs::read_to_string(dir.join("world-edits.json")).unwrap();
    assert!(after.contains("batch/keeper-pad"), "batch persisted");
    assert!(after.ends_with('\n'), "canonical trailing newline");
    let out = tmp("edits-loop-build");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(r.status.success(), "persisted script builds green");
}

/// A red candidate is rejected by BOTH subcommands and `apply` persists
/// nothing — an editing session can never leave a broken script behind.
#[test]
fn edit_apply_rejects_a_red_candidate_without_persisting() {
    let dir = edits_copy("edits-red");
    let before = std::fs::read_to_string(dir.join("world-edits.json")).unwrap();
    let batch_file = tmp("edits-red-batch").with_extension("json");
    std::fs::write(
        &batch_file,
        serde_json::to_string_pretty(&serde_json::json!({
            "id": "batch/breach-wall",
            "area": "area/keep",
            "edits": [
                { "verb": "select", "name": "region/breach", "shape": {
                    "kind": "box",
                    "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                    "min": [4, 1, 0], "max": [6, 2, 0]
                }},
                { "verb": "carve", "region": "region/breach" }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let shots = tmp("edits-red-shots");
    let r = delvec(&[
        "edit",
        "apply",
        dir.to_str().unwrap(),
        "--batch",
        batch_file.to_str().unwrap(),
        "-o",
        shots.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert_eq!(r.status.code(), Some(3), "boundary breach rejected");
    let stdout = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(stdout.contains("DW0322"), "expected DW0322:\n{stdout}");
    assert_eq!(
        std::fs::read_to_string(dir.join("world-edits.json")).unwrap(),
        before,
        "a red apply persists nothing"
    );
}

/// `delvec snapshot` shows the EDITED world (view mode): the corner terrace
/// raised by the fixture's morph batch is present in the assembled snapshot
/// state — the loop's read half sees what the write half did.
#[test]
fn snapshot_renders_the_edited_world() {
    let dir = edits_fixture_dir();
    let png = tmp("edits-snap").with_extension("png");
    let r = delvec(&[
        "snapshot",
        dir.to_str().unwrap(),
        "--camera",
        // Straight down over the raised outer corner (world [1..2, 65, 1..2]).
        "1.5,72,1.5,0,90",
        "-o",
        png.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(
        r.status.success(),
        "snapshot failed:\n{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    let manifest =
        std::fs::read_to_string(png.with_extension("manifest.json")).expect("manifest written");
    let doc: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    // The raised terrace lifts the world's max block count/kinds; assert the
    // world block kinds include the morph recipe's bricks via the kind count
    // being > the unedited room's 4 (stone, iron_bars, lantern, air is absent).
    let kinds = doc["world"]["block_kinds"].as_u64().unwrap();
    assert!(
        kinds >= 6,
        "edited world carries the recipe palettes: {kinds}"
    );
}
