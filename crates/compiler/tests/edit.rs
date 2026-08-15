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
    // ORDER, not mere presence (map-editor audit nit). The runtime order is a
    // load-bearing contract: `world_edits` must land AFTER the socket seals
    // (which overwrite raw structure blocks) and BEFORE the relight fixtures
    // (measured over the edited world) and every entity/hardware setup step —
    // that ordering is exactly what makes an edit over a trap's dispenser
    // (`DW0352`) silently fatal. A `contains` assertion proved none of it.
    let lines: Vec<&str> = setup_finish.lines().collect();
    let at = |pred: &dyn Fn(&str) -> bool| lines.iter().position(|l| pred(l));
    let edits = at(&|l: &str| l == "function hello-world:world_edits")
        .expect("setup_finish calls world_edits");
    let summon = at(&|l: &str| l.starts_with("summon ")).expect("the fixture summons the keeper");
    let release = at(&|l: &str| l.starts_with("forceload remove "))
        .expect("out-of-bbox edit chunks released");
    assert_eq!(
        edits, 0,
        "world_edits is the first write of setup_finish — it must land on the \
         raw placed structures, after the socket seals (none in this \
         single-piece fixture) and before every later pass:\n{setup_finish}"
    );
    assert!(
        edits < summon,
        "world_edits runs BEFORE anything is summoned into the edited geometry:\n{setup_finish}"
    );
    assert!(
        summon < release,
        "the one-shot edit forceloads are released only after every write:\n{setup_finish}"
    );
    assert_eq!(
        lines.last().copied(),
        Some("scoreboard players set #placed dw.sys 1"),
        "the `#placed` latch is the last line, so nothing runs after it"
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

/// Rewrite the copy's stage-1 world doc to declare `horizon: ocean` (spec-0013),
/// leaving everything else in the fixture alone.
fn set_ocean_horizon(dir: &Path) {
    let path = dir.join("world.json");
    let mut doc: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    doc["dsl_version"] = serde_json::json!("0.6.0");
    doc["content"]["horizon"] = serde_json::json!("ocean");
    // `DW0320`: an ocean horizon needs a return rule; the default margin is fine.
    doc["content"]["boundary"] = serde_json::json!({});
    std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap()).unwrap();
}

/// Boundary safety on an **ocean** horizon (`DW0322`), the false-premise fix:
/// the pinned bedrock/stone/water superflat puts ground under every column, so
/// nothing in an ocean world can fall out of it and the void-drop premise is
/// vacuous. A zero-write `select` — the map editor's probe batch, which used to
/// trip `DW0322` on every coastline of `nobodys-cave-island` — must build clean.
#[test]
fn edit_select_only_batch_on_an_ocean_horizon_is_green() {
    let dir = edits_copy("edits-ocean-select");
    set_ocean_horizon(&dir);
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/probe",
            "area": "area/keep",
            "edits": [
                { "verb": "select", "name": "region/probe", "shape": {
                    "kind": "box",
                    "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                    "min": [4, 1, 4], "max": [5, 2, 5]
                }}
            ]
        }]),
    );
    let out = tmp("edits-ocean-select-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    let stdout = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert_eq!(r.status.code(), Some(0), "ocean probe batch:\n{stdout}");
    assert!(!stdout.contains("DW0322"), "no boundary error:\n{stdout}");
}

/// The ocean horizon's *replacement* invariant (`DW0322`): the same wall breach
/// that is a void drop under `horizon: void` is a **stranding** hazard under
/// `horizon: ocean` — the room's floor sits below sea level, so a player who
/// walks out of the breach is in open water with no shoreline at the waterline
/// to climb back onto. The code is the same, the premise and the prescription
/// are the horizon's.
#[test]
fn edit_ocean_breach_strands_the_player_dw0322() {
    let dir = edits_copy("edits-ocean-breach");
    set_ocean_horizon(&dir);
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
    let out = tmp("edits-ocean-breach-out");
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
    assert!(
        stdout.contains("NO way back ashore"),
        "the ocean horizon reports stranding, not a void drop:\n{stdout}"
    );
    assert!(
        !stdout.contains("void drop"),
        "the void premise must not be asserted in an ocean world:\n{stdout}"
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

/// The editor's per-batch manifest carries the **layout** (`pieces`), and that
/// listing is by itself sufficient to author a `piece-local` frame: this test
/// reads `index`, `prefab` and `size` straight out of a rendered manifest, builds
/// a frame from nothing else, and the replay resolves it. Nothing here is
/// back-solved from the geometry — which is what an editor had to do before the
/// listing existed (the manifest carried anchors and area bounds only).
#[test]
fn the_batch_manifest_pieces_listing_resolves_a_piece_local_frame() {
    let dir = edits_copy("edits-pieces");
    let shots = tmp("edits-pieces-shots");
    let r = delvec(&[
        "edit",
        "preview",
        dir.to_str().unwrap(),
        "-o",
        shots.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(
        r.status.success(),
        "baseline preview failed:\n{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(shots.join("dress-floor.manifest.json")).expect("batch manifest"),
    )
    .expect("manifest is JSON");
    let piece = manifest["pieces"]
        .as_array()
        .expect("pieces listing")
        .iter()
        .find(|p| p["area"] == "area/keep")
        .expect("the keep's piece is listed")
        .clone();
    let index = piece["index"].as_u64().expect("index");
    let prefab = piece["prefab"].as_str().expect("prefab").to_string();
    let size = piece["size"].as_array().expect("size");
    let hi: Vec<i64> = size.iter().map(|v| v.as_i64().unwrap() - 1).collect();

    // A frame authored purely from the listing: the piece's whole local box.
    let batch_file = tmp("edits-pieces-batch").with_extension("json");
    std::fs::write(
        &batch_file,
        serde_json::to_string_pretty(&serde_json::json!({
            "id": "batch/from-manifest",
            "area": "area/keep",
            "edits": [
                { "verb": "select", "name": "region/whole-piece", "shape": {
                    "kind": "box",
                    "frame": { "kind": "piece-local", "piece": index, "prefab": prefab },
                    "min": [0, 0, 0], "max": [hi[0], hi[1], hi[2]]
                }}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
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
    let stdout = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(
        r.status.success(),
        "a frame built only from the manifest listing must resolve:\n{stdout}"
    );
    assert!(!stdout.contains("DW0323"), "no frame drift:\n{stdout}");
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
    // Atomic persist (tmp + rename): the artifact of record is never left
    // half-written, and no scratch file is left in the campaign directory.
    assert!(
        !dir.join("world-edits.json.tmp").exists(),
        "the temp file is renamed away, never left behind"
    );
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

/// PR 2 verbs materialize through the same `world_edits` function: the
/// scatter's dressing, the planted oak (logs + persistent leaves), the
/// stamped fragment and the baked relight torch all lower to `fill`/`setblock`
/// lines — and `setup` forceloads every batch's write AABB so a write that
/// leaves the piece bboxes (a leaning canopy) cannot silently fail on an
/// unloaded chunk.
#[test]
fn pr2_verbs_materialize_and_their_bounds_are_forceloaded() {
    let dir = edits_fixture_dir();
    let out = tmp("edits-pr2-emit");
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
        "build failed:\n{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    let body = std::fs::read_to_string(
        out.join("datapack/data/hello-world/function/world_edits.mcfunction"),
    )
    .expect("world_edits.mcfunction emitted");
    assert!(body.contains("minecraft:oak_log[axis=y]"), "plant: trunk");
    assert!(
        body.contains("minecraft:oak_leaves[persistent=true]"),
        "plant: persistent leaves"
    );
    assert!(body.contains("minecraft:torch"), "relight: baked fixture");
    assert!(
        body.contains("minecraft:poppy") || body.contains("minecraft:oxeye_daisy"),
        "scatter: dressing"
    );
    // The fragment stamp (a full hello-room copy 7 blocks down) contributes
    // its iron-bars door row at y = 64 - 7 + 1..3.
    assert!(
        body.contains("minecraft:iron_bars"),
        "fragment: stamped annex content"
    );
    let setup =
        std::fs::read_to_string(out.join("datapack/data/hello-world/function/setup.mcfunction"))
            .expect("setup emitted");
    let forceloads = setup.matches("forceload add").count();
    assert!(
        forceloads > 1,
        "edit-write AABBs are forceloaded beside the piece bboxes ({forceloads})"
    );
}

/// A `fragment` naming a prefab outside the admitted library is a loud
/// `DW0323` — only library prefabs (with ADR-0013 provenance/license
/// metadata) can be stamped.
#[test]
fn fragment_outside_the_library_is_dw0323() {
    let dir = edits_copy("edits-frag-unknown");
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/bad-stamp",
            "area": "area/keep",
            "edits": [
                { "verb": "fragment", "prefab": "prefab/not-in-library",
                  "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                  "at": [0, -7, 0] }
            ]
        }]),
    );
    let out = tmp("edits-frag-unknown-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert_eq!(r.status.code(), Some(3));
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    );
    assert!(all.contains("DW0323"), "expected DW0323:\n{all}");
    assert!(
        all.contains("prefab/not-in-library"),
        "names the prefab:\n{all}"
    );
}

// ---------------------------------------------------------------------------
// Map-editor audit fixes (findings 1–6). Each of these fails on pre-fix output
// — verified against the origin/main `delvec` before the fix landed.
// ---------------------------------------------------------------------------

/// A piece-local box `select` over the fixture's single room.
fn piece_box(name: &str, min: [i32; 3], max: [i32; 3]) -> serde_json::Value {
    serde_json::json!({
        "verb": "select", "name": name, "shape": {
            "kind": "box",
            "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
            "min": min, "max": max
        }
    })
}

/// Bump a copy's `world`/`quests` stage documents to 0.6.0 and replace the
/// quest content — the v0.6 surfaces (traps, `close-gate`, cutscenes) the audit
/// checks need are not expressible in the fixture's 0.2.0 stages.
fn set_quests_v06(dir: &Path, content: serde_json::Value) {
    for (file, doc) in [("world.json", "world"), ("quests.json", "quests")] {
        let path = dir.join(file);
        let mut v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        v["dsl_version"] = serde_json::json!("0.6.0");
        if doc == "quests" {
            v["content"] = content.clone();
        }
        std::fs::write(&path, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    }
}

/// The fixture's base quest content, with `extra` merged into `content`.
fn quests_with(extra: serde_json::Value) -> serde_json::Value {
    let mut content = serde_json::json!({
        "quests": [ {
            "id": "quest/open-the-door",
            "trigger": { "type": "campaign-start" },
            "objectives": [
                { "type": "talk-to", "id": "obj/talk", "npc": "npc/keeper" },
                { "type": "reach-anchor", "id": "obj/exit", "anchor": "anchor/exit",
                  "radius": 2, "after": ["obj/talk"] }
            ],
            "on_objective_complete": {
                "obj/talk": [ { "type": "open-gate", "anchor": "anchor/door" } ]
            },
            "on_complete": [ { "type": "campaign-complete" } ]
        } ]
    });
    for (k, v) in extra.as_object().unwrap() {
        if k == "on_complete" {
            content["quests"][0]["on_complete"] = v.clone();
        } else {
            content[k] = v.clone();
        }
    }
    content
}

/// A private prefab copy whose `hello-room.json` exposes an `anchor/trap` whose
/// trigger cell is `[7, 1, 6]` and dispenser socket `[8, 1, 6]` — exactly the
/// two cells the fixture's own `batch/hearth-nook` carves.
fn prefabs_with_trap(name: &str) -> PathBuf {
    let dir = tmp(name);
    common::copy_dir_all(&common::prefabs_dir(), &dir);
    let path = dir.join("hello-room.json");
    let mut meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    meta["anchors"]["anchor/trap"] =
        serde_json::json!({ "pos": [7, 1, 6], "dispenser": [8, 1, 6] });
    std::fs::write(&path, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    dir
}

fn combined(r: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&r.stdout),
        String::from_utf8_lossy(&r.stderr)
    )
}

/// **Finding 1 (`DW0352`).** `setup_finish` runs `world_edits` BEFORE
/// `trap_setup`, so a batch that carves a trap's trigger/dispenser cell lands
/// first and the trap is then loaded into a block that is no longer there:
/// vanilla's `item replace block … container.0` on a non-container fails with
/// NO output, shipping a dead trap while every geometry proof stays green (the
/// pre-fix binary emitted exactly this: `fill 7 65 6 8 65 6 minecraft:air`
/// followed by `item replace block 8 65 6 container.0 …`, exit 0).
#[test]
fn edit_over_trap_hardware_is_dw0352() {
    let dir = edits_copy("edits-trap");
    let prefabs = prefabs_with_trap("edits-trap-prefabs");
    set_quests_v06(
        &dir,
        quests_with(serde_json::json!({
            "traps": [ {
                "id": "trap/dart", "at": "anchor/trap", "trigger": "pressure-plate",
                "effect": { "dispense": { "item": "minecraft:arrow", "count": 8 } },
                "lethality": "harmful"
            } ]
        })),
    );
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/hearth-nook",
            "area": "area/keep",
            "edits": [
                piece_box("region/nook", [7, 1, 6], [8, 2, 6]),
                { "verb": "carve", "region": "region/nook" }
            ]
        }]),
    );
    let out = tmp("edits-trap-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert_eq!(r.status.code(), Some(3), "build-tier failure");
    let stdout = combined(&r);
    assert!(stdout.contains("DW0352"), "expected DW0352:\n{stdout}");
    assert!(
        stdout.contains("batch/hearth-nook") && stdout.contains("trap/dart"),
        "names the batch AND the trap:\n{stdout}"
    );

    // The same script with the edit moved off the trap's cells builds green —
    // the check is a collision test, not a blanket ban on editing near traps.
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/hearth-nook",
            "area": "area/keep",
            "edits": [
                piece_box("region/nook", [1, 1, 2], [1, 2, 2]),
                { "verb": "carve", "region": "region/nook" }
            ]
        }]),
    );
    let out = tmp("edits-trap-ok");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        prefabs.to_str().unwrap(),
    ]);
    assert!(r.status.success(), "clear of the trap:\n{}", combined(&r));
    // …and the ordering the finding is about is pinned here: the trap hardware
    // is loaded strictly AFTER `world_edits` has run, which is precisely why a
    // colliding edit is silently fatal and must be a build error.
    let finish = std::fs::read_to_string(
        out.join("datapack/data/hello-world/function/setup_finish.mcfunction"),
    )
    .unwrap();
    let lines: Vec<&str> = finish.lines().collect();
    let edits = lines
        .iter()
        .position(|l| *l == "function hello-world:world_edits")
        .expect("world_edits called");
    let hardware = lines
        .iter()
        .position(|l| l.starts_with("item replace block ") && l.contains("container.0"))
        .expect("trap dispenser loaded");
    assert!(
        edits < hardware,
        "world_edits runs BEFORE trap_setup:\n{finish}"
    );
}

/// **Finding 2 + 6.** An edit AABB outside every piece bbox gets its own
/// per-chunk `execute if loaded` convergence sentinel folded into `#placeok`
/// (so `setup_finish` — and therefore the one-shot `world_edits` — cannot run
/// into a still-loading chunk and lose those writes forever), and the chunks it
/// forceloaded are released at the END of `setup_finish` once the writes have
/// landed. Piece forceloads are never released: the gameplay tick machinery
/// keeps addressing them. Pre-fix, `place_verify` held only the piece sentinel
/// and nothing was ever released.
#[test]
fn out_of_bbox_edit_chunks_converge_then_release() {
    let dir = edits_copy("edits-farchunk");
    // A fragment stamped 58 blocks out spans chunks (3,3)..(4,4) — four chunks,
    // none of them the room's chunk (0,0).
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/far-annex",
            "area": "area/keep",
            "edits": [ {
                "verb": "fragment", "prefab": "prefab/hello-room",
                "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
                "at": [58, -7, 58]
            } ]
        }]),
    );
    let out = tmp("edits-farchunk-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(r.status.success(), "build failed:\n{}", combined(&r));

    let fnpath = |n: &str| out.join(format!("datapack/data/hello-world/function/{n}.mcfunction"));
    let verify = std::fs::read_to_string(fnpath("place_verify")).unwrap();
    let loaded: Vec<&str> = verify
        .lines()
        .filter(|l| l.starts_with("execute if loaded "))
        .collect();
    assert_eq!(
        loaded,
        vec![
            "execute if loaded 58 57 58 run scoreboard players add #placeok dw.sys 1",
            "execute if loaded 58 57 64 run scoreboard players add #placeok dw.sys 1",
            "execute if loaded 64 57 58 run scoreboard players add #placeok dw.sys 1",
            "execute if loaded 64 57 64 run scoreboard players add #placeok dw.sys 1",
        ],
        "one load sentinel per out-of-bbox edit chunk, deterministic order"
    );
    // The gate counts them: 4 chunks + 1 piece sentinel.
    assert!(
        verify.contains("execute if score #placeok dw.sys matches 5 run function"),
        "edit chunks are part of the convergence gate:\n{verify}"
    );

    let finish = std::fs::read_to_string(fnpath("setup_finish")).unwrap();
    let removes: Vec<&str> = finish
        .lines()
        .filter(|l| l.starts_with("forceload remove "))
        .collect();
    assert_eq!(
        removes,
        vec![
            "forceload remove 58 58",
            "forceload remove 58 64",
            "forceload remove 64 58",
            "forceload remove 64 64",
        ],
        "the one-shot edit chunks are released"
    );
    let lines: Vec<&str> = finish.lines().collect();
    let first_remove = lines
        .iter()
        .position(|l| l.starts_with("forceload remove "))
        .unwrap();
    let write = lines
        .iter()
        .position(|l| l.contains("hello-world:world_edits"))
        .unwrap();
    assert!(
        write < first_remove,
        "release happens after the writes:\n{finish}"
    );
    assert_eq!(
        lines.last().copied(),
        Some("scoreboard players set #placed dw.sys 1"),
        "release sits at the very end of setup_finish, after every other write"
    );
    // The piece bbox forceload is untouched — gameplay needs it all session.
    let setup = std::fs::read_to_string(fnpath("setup")).unwrap();
    assert!(setup.contains("forceload add 0 0 10 10"), "{setup}");
    assert!(
        !finish.contains("forceload remove 0 0"),
        "piece forceloads are never released:\n{finish}"
    );
}

/// **Finding 3.** `edit apply` used to persist on the per-batch invariant
/// SUBSET, so a script `build` rejects could be written into the campaign
/// (proven against the pre-fix binary: it persisted this batch, and the very
/// next `build` failed `DW0308`). `apply` now runs the whole build-tier proof
/// set before persisting.
#[test]
fn edit_apply_runs_the_full_build_tier_proof_set() {
    let dir = edits_copy("edits-fullproof");
    set_quests_v06(
        &dir,
        quests_with(serde_json::json!({
            "on_complete": [
                { "type": "cutscene", "seconds": 3, "path": [
                    { "anchor": "anchor/keeper-stand", "offset": [0, 2, 0] },
                    { "anchor": "anchor/exit", "offset": [0, 2, 0] }
                ] },
                { "type": "campaign-complete" }
            ]
        })),
    );
    set_batches(&dir, serde_json::json!([]));
    let before = std::fs::read_to_string(dir.join("world-edits.json")).unwrap();

    // A one-block fill in the dolly's flight path: invisible to the per-batch
    // invariants (it is above head height, so walkability, boundary safety,
    // gravity and relight all stay green) but a hard `DW0308` cutscene clip.
    let batch_file = tmp("edits-fullproof-batch").with_extension("json");
    std::fs::write(
        &batch_file,
        serde_json::to_string_pretty(&serde_json::json!({
            "id": "batch/clip",
            "area": "area/keep",
            "edits": [
                piece_box("region/c", [5, 3, 7], [5, 3, 7]),
                { "verb": "fill", "region": "region/c", "recipe": {
                    "blocks": [ { "block": "minecraft:stone", "weight": 1.0 } ] } }
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let shots = tmp("edits-fullproof-shots");
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
    assert_eq!(r.status.code(), Some(3), "apply rejects it");
    let stdout = combined(&r);
    assert!(stdout.contains("DW0308"), "expected DW0308:\n{stdout}");
    assert_eq!(
        std::fs::read_to_string(dir.join("world-edits.json")).unwrap(),
        before,
        "a batch `build` would reject is never persisted"
    );
}

/// **Finding 5a (`DW0353`, advisory).** A write inside a `close-gate` region
/// survives every proof but is erased by the first close/open cycle.
#[test]
fn edit_inside_a_close_gate_region_warns_dw0353() {
    let dir = edits_copy("edits-gate");
    set_quests_v06(
        &dir,
        quests_with(serde_json::json!({
            "on_complete": [
                { "type": "close-gate", "anchor": "anchor/door" },
                { "type": "campaign-complete" }
            ]
        })),
    );
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/gate-dressing",
            "area": "area/keep",
            "edits": [
                piece_box("region/g", [4, 1, 6], [5, 3, 6]),
                { "verb": "fill", "region": "region/g", "recipe": {
                    "blocks": [ { "block": "minecraft:air", "weight": 1.0 } ] } }
            ]
        }]),
    );
    let out = tmp("edits-gate-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    let stdout = combined(&r);
    assert!(
        r.status.success(),
        "advisory only — never fails the build:\n{stdout}"
    );
    assert!(stdout.contains("DW0353"), "expected DW0353:\n{stdout}");
    assert!(stdout.contains("[warning]"), "warning tier:\n{stdout}");
    assert!(
        stdout.contains("batch/gate-dressing"),
        "names the batch:\n{stdout}"
    );
}

/// **Finding 5b (`DW0354`, advisory).** `scatter` dropping flowers onto bare
/// stone: vanilla pops every one of them off on the first chunk tick, so the
/// dressing the author sees in `delvec snapshot` never reaches players.
#[test]
fn scatter_on_non_soil_warns_dw0354() {
    let dir = edits_copy("edits-flora");
    set_batches(
        &dir,
        serde_json::json!([{
            "id": "batch/flowers-on-stone",
            "area": "area/keep",
            "edits": [
                piece_box("region/f", [1, 0, 7], [3, 0, 9]),
                { "verb": "select", "name": "region/air", "shape": {
                    "kind": "surface-band", "over": "region/f", "from": 1, "to": 1 } },
                { "verb": "scatter", "region": "region/air", "density": 1.0,
                  "items": [ { "block": "minecraft:poppy", "weight": 1.0 } ] }
            ]
        }]),
    );
    let out = tmp("edits-flora-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    let stdout = combined(&r);
    assert!(r.status.success(), "advisory only:\n{stdout}");
    assert!(stdout.contains("DW0354"), "expected DW0354:\n{stdout}");
    assert!(stdout.contains("[warning]"), "warning tier:\n{stdout}");
    assert!(
        stdout.contains("flowers cannot root in"),
        "names the reason:\n{stdout}"
    );
}

/// **Finding 5b (`DW0354`, error tier).** A fixture the script's own `relight`
/// verb placed is a DECLARED `min_light` guarantee: a later batch that carves
/// its support drops the light source on the floor and silently re-darkens a
/// region `DW0211` accepted. That one is an error, not advice.
#[test]
fn carving_a_relight_fixtures_support_is_a_dw0354_error() {
    let dir = edits_copy("edits-fixture-support");
    set_batches(
        &dir,
        serde_json::json!([
            { "id": "batch/pillar", "area": "area/keep", "edits": [
                piece_box("region/p", [2, 1, 8], [2, 1, 8]),
                { "verb": "fill", "region": "region/p", "recipe": {
                    "blocks": [ { "block": "minecraft:stone", "weight": 1.0 } ] } }
            ]},
            { "id": "batch/lamp", "area": "area/keep", "edits": [
                piece_box("region/l", [2, 2, 8], [2, 2, 8]),
                { "verb": "relight", "region": "region/l", "fixture": "torch", "min_light": 14 }
            ]},
            { "id": "batch/undermine", "area": "area/keep", "edits": [
                piece_box("region/u", [2, 1, 8], [2, 1, 8]),
                { "verb": "carve", "region": "region/u" }
            ]}
        ]),
    );
    let out = tmp("edits-fixture-support-out");
    let r = delvec(&[
        "build",
        dir.to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert_eq!(r.status.code(), Some(3), "error tier, not advisory");
    let stdout = combined(&r);
    assert!(stdout.contains("DW0354"), "expected DW0354:\n{stdout}");
    assert!(
        stdout.contains("batch/undermine") && stdout.contains("relight"),
        "names the batch that broke it and what broke:\n{stdout}"
    );
}

/// **Finding 5b, root cause.** `fragment` stamps a library prefab's blocks as
/// the actual runtime `setblock` writes, so it must keep their BLOCKSTATE. It
/// used to read bare ids, turning `hello-room`'s `lantern[hanging=true]` into a
/// floor lantern stamped into mid-air — a defect `DW0354` surfaced and nothing
/// else could see.
#[test]
fn fragment_stamps_preserve_blockstate() {
    let out = tmp("edits-fragment-state");
    let r = delvec(&[
        "build",
        edits_fixture_dir().to_str().unwrap(),
        "-o",
        out.to_str().unwrap(),
        "--prefabs",
        &prefabs_arg(),
    ]);
    assert!(r.status.success(), "build failed:\n{}", combined(&r));
    let body = std::fs::read_to_string(
        out.join("datapack/data/hello-world/function/world_edits.mcfunction"),
    )
    .unwrap();
    assert!(
        body.contains("minecraft:lantern[hanging=true]"),
        "the stamped lantern keeps `hanging=true`:\n{body}"
    );
    assert!(
        !body.lines().any(|l| l.ends_with("minecraft:lantern")),
        "no bare (floor) lantern is stamped:\n{body}"
    );
}

/// **Rotated fragment stamps are REFUSED, not warned about** (planner ruling on
/// the gap `fragment_stamps_preserve_blockstate` surfaced). `rotation` turns
/// cell POSITIONS only — there is no rotate-aware blockstate rewriter — so a
/// quarter-turned stamp of a prefab carrying `facing`/`axis`/`shape`/connection
/// state would ship visibly deformed geometry. That is the silently-deformed-map
/// class, so it is a build error (`DW0323`, the fragment verb's resolution code).
///
/// It is a **collision test, not a blanket ban**: a prefab whose every state is
/// yaw-invariant rotates correctly and stays allowed. Rejecting that too would
/// be a check asserting something false. That half is proved against
/// [`prefabs_with_a_yaw_invariant_piece`], a piece built to have the property
/// rather than a shipped one observed to — see its own note for why no shipped
/// piece can serve.
#[test]
fn rotated_fragment_of_a_directional_prefab_is_dw0323() {
    let dir = edits_copy("edits-rotation");
    let prefabs = prefabs_with_a_yaw_invariant_piece();
    let stamp = |prefab: &str, rotation: Option<&str>| {
        let mut edit = serde_json::json!({
            "verb": "fragment", "prefab": prefab,
            "frame": { "kind": "piece-local", "piece": 0, "prefab": "prefab/hello-room" },
            "at": [0, -9, 0]
        });
        if let Some(r) = rotation {
            edit["rotation"] = serde_json::json!(r);
        }
        set_batches(
            &dir,
            serde_json::json!([{ "id": "batch/rot", "area": "area/keep", "edits": [edit] }]),
        );
        delvec(&[
            "build",
            dir.to_str().unwrap(),
            "-o",
            tmp("edits-rotation-out").to_str().unwrap(),
            "--prefabs",
            prefabs.to_str().unwrap(),
        ])
    };

    // `keep-stair` carries `facing`/`shape` stair state: a quarter-turn would
    // leave every stair pointing the way it did before it moved.
    let r = stamp("prefab/keep-stair", Some("clockwise90"));
    assert_eq!(r.status.code(), Some(3), "build-tier refusal");
    let stdout = combined(&r);
    assert!(stdout.contains("DW0323"), "expected DW0323:\n{stdout}");
    assert!(
        stdout.contains("prefab/keep-stair") && stdout.contains("facing="),
        "names the prefab and the offending property:\n{stdout}"
    );
    assert!(
        stdout.contains("rotate-aware stamping is NOT implemented"),
        "says the compiler refuses, and why:\n{stdout}"
    );

    // The same prefab stamped unrotated is fine — the refusal is about the
    // rotation, not about the prefab.
    let r = stamp("prefab/keep-stair", None);
    assert!(r.status.success(), "unrotated is fine:\n{}", combined(&r));

    // And a prefab with no yaw-dependent state rotates correctly, in every
    // direction: the check must not reject provably correct output.
    for rot in ["clockwise90", "counterclockwise90", "clockwise180"] {
        let r = stamp("prefab/yaw-invariant-box", Some(rot));
        assert!(
            r.status.success(),
            "yaw-invariant prefab rotates fine ({rot}):\n{}",
            combined(&r)
        );
    }

    // The shipped gate room is the other side of the same coin: its grille is
    // an east–west run of bars, so a quarter-turn deforms it and the check says
    // so. This is the case the fixture above used to stand in for.
    let r = stamp("prefab/hello-room", Some("clockwise90"));
    assert_eq!(
        r.status.code(),
        Some(3),
        "a barred gate is not yaw-invariant"
    );
    assert!(
        combined(&r).contains("minecraft:iron_bars"),
        "names the grille:\n{}",
        combined(&r)
    );
}

/// The shipped library, plus one piece that is yaw-invariant **by
/// construction**: a stone box, a hanging lantern and an upright log — states
/// that carry properties (`hanging`, `waterlogged`, `axis=y`) and not one of
/// them a direction a quarter-turn can move.
///
/// It has to be built rather than borrowed, and the reason is the finding. This
/// half of the test used to stamp `hello-room`, which measured as yaw-invariant
/// only because its iron-bars gate wrote **no connection properties at all**.
/// The grille has always run east–west, so a quarter-turn had always deformed
/// it; `DW0323` could not see that because the file did not say. Now that every
/// emitted state names what it joins, no shipped piece is yaw-invariant — and a
/// check whose "provably-correct output is accepted" half has no case left is a
/// blanket ban that nobody can tell apart from a rule.
fn prefabs_with_a_yaw_invariant_piece() -> PathBuf {
    use fastnbt::Value;
    let dir = tmp("edits-rotation-prefabs");
    common::copy_dir_all(&common::prefabs_dir(), &dir);

    let (sx, sy, sz) = (5i32, 4i32, 5i32);
    let mut blocks: Vec<Value> = Vec::new();
    for x in 0..sx {
        for y in 0..sy {
            for z in 0..sz {
                let shell = y == 0 || y == sy - 1 || x == 0 || x == sx - 1 || z == 0 || z == sz - 1;
                let state = if [x, y, z] == [2, sy - 2, 2] {
                    2 // the lantern, hung from the middle of the ceiling
                } else if [x, y, z] == [1, 1, 1] {
                    3 // an upright log: `axis=y` is the excluded-by-name case
                } else if shell {
                    1
                } else {
                    0
                };
                let mut c = std::collections::HashMap::new();
                c.insert(
                    "pos".to_string(),
                    Value::List(vec![Value::Int(x), Value::Int(y), Value::Int(z)]),
                );
                c.insert("state".to_string(), Value::Int(state));
                blocks.push(Value::Compound(c));
            }
        }
    }
    let entry = |name: &str, props: &[(&str, &str)]| {
        let mut c = std::collections::HashMap::new();
        c.insert("Name".to_string(), Value::String(name.to_string()));
        if !props.is_empty() {
            let mut p = std::collections::HashMap::new();
            for (k, v) in props {
                p.insert(k.to_string(), Value::String(v.to_string()));
            }
            c.insert("Properties".to_string(), Value::Compound(p));
        }
        Value::Compound(c)
    };
    // Every state this fixture authors, judged against the pinned registry
    // before the bytes exist. A palette nobody judges can name a block 1.21.11
    // does not have, and a structure template loads an unknown block as AIR —
    // so the piece would be a hole and the test would still be green.
    let states: &[(&str, &[(&str, &str)])] = &[
        ("minecraft:air", &[]),
        ("minecraft:stone", &[]),
        (
            "minecraft:lantern",
            &[("hanging", "true"), ("waterlogged", "false")],
        ),
        ("minecraft:oak_log", &[("axis", "y")]),
    ];
    let registry = delvewright_schem::blocks::BlockRegistry::v1_21_11();
    for (name, props) in states {
        let props: BTreeMap<String, String> = props
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let verdict = registry.validate(name, &props);
        verdict
            .unwrap_or_else(|e| panic!("the yaw-invariant fixture names an impossible state: {e}"));
    }

    let mut root = std::collections::HashMap::new();
    root.insert("DataVersion".to_string(), Value::Int(4671));
    root.insert(
        "size".to_string(),
        Value::List(vec![Value::Int(sx), Value::Int(sy), Value::Int(sz)]),
    );
    root.insert(
        "palette".to_string(),
        Value::List(states.iter().map(|(n, p)| entry(n, p)).collect()),
    );
    root.insert("blocks".to_string(), Value::List(blocks));
    root.insert("entities".to_string(), Value::List(vec![]));
    let raw = fastnbt::to_bytes(&Value::Compound(root)).expect("nbt");
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::Write::write_all(&mut gz, &raw).expect("gzip");
    std::fs::write(
        dir.join("yaw-invariant-box.nbt"),
        gz.finish().expect("gzip"),
    )
    .expect("write");

    std::fs::write(
        dir.join("yaw-invariant-box.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "prefab_id": "prefab/yaw-invariant-box",
            "structure": {
                "file": "yaw-invariant-box.nbt",
                "id": "yaw-invariant-box",
                "size": [sx, sy, sz],
                "data_version": 4671,
                "generator": "crates/compiler/tests/edit.rs (test fixture)"
            },
            "anchors": {},
            "connectors": []
        }))
        .expect("json"),
    )
    .expect("write");
    dir
}
