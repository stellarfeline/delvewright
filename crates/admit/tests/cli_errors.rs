//! CLI-level error-path tests: input, tooling, dark-lighting-advisory, and
//! gallery-write diagnostics. Complements `tests/cli.rs` (happy-path chains)
//! and `tests/audit.rs`/`tests/catalog.rs` (already cover `DW0730`/`DW0731`/
//! `DW0740`/`DW0741` at the library level).

use std::path::PathBuf;
use std::process::Command;

use delvewright_admit::fixtures;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_delve-admit")
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delve-admit-err-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `DW0732`: an unreadable input file — `audit` on a path that does not exist.
#[test]
fn audit_missing_file_is_dw0732_exit2() {
    let dir = tmp("missing-input");
    let out = Command::new(bin())
        .arg("audit")
        .arg(dir.join("nonexistent.nbt"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DW0732"), "expected DW0732: {stderr}");
}

/// `DW0750`: admission tooling failure — `socket` carving a jigsaw opening
/// outside the structure's bounds.
#[test]
fn socket_out_of_bounds_is_dw0750_exit1() {
    let dir = tmp("socket-oob");
    let nbt = dir.join("piece.nbt");
    std::fs::write(&nbt, fixtures::clean_room().write()).unwrap();

    let out = Command::new(bin())
        .args(["socket"])
        .arg(&nbt)
        .args(["--pos", "999,999,999", "--facing", "north"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DW0750"), "expected DW0750: {stderr}");
}

/// `DW0751`: the lighting probe's dark-interior advisory — measured, printed,
/// but does not fail the command (spec-0010: advisory only, no longer gates).
#[test]
fn lighting_dark_room_is_dw0751_advisory_exit0() {
    let dir = tmp("dark-room");
    let nbt = dir.join("dark.nbt");
    std::fs::write(&nbt, fixtures::dark_room().write()).unwrap();

    let out = Command::new(bin())
        .args(["lighting"])
        .arg(&nbt)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "the dark advisory must not fail the command: {out:?}"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DW0751"), "expected DW0751: {stderr}");
}

/// `DW0760`: gallery emission failure — the output path is blocked by an
/// existing regular file, so the gallery tree cannot be written.
#[test]
fn gallery_output_blocked_is_dw0760_exit3() {
    let dir = tmp("gallery-blocked");
    let candidates = dir.join("candidates");
    std::fs::create_dir_all(&candidates).unwrap();
    std::fs::write(
        candidates.join("gatehouse.nbt"),
        fixtures::clean_room().write(),
    )
    .unwrap();

    // Block the gallery output root with a regular file.
    let out_dir = dir.join("out-is-a-file");
    std::fs::write(&out_dir, b"blocker").unwrap();

    let out = Command::new(bin())
        .args(["gallery"])
        .arg(&candidates)
        .args(["-o"])
        .arg(&out_dir)
        .args(["--id", "demo"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DW0760"), "expected DW0760: {stderr}");
}

/// Stage a two-tile zone: two real `.nbt` files and the manifest naming them.
fn stage_tile_set(dir: &std::path::Path) -> PathBuf {
    let room = fixtures::clean_room();
    let depth = room.size[2];
    let parts: Vec<serde_json::Value> = (0..2)
        .map(|i| {
            let file = format!("zone.x0y0z{i}.nbt");
            std::fs::write(dir.join(&file), fixtures::clean_room().write()).unwrap();
            serde_json::json!({
                "file": file,
                "id": format!("zone.x0y0z{i}"),
                "grid_index": [0, 0, i],
                "offset": [0, 0, i * depth],
                "size": room.size,
            })
        })
        .collect();
    let manifest = dir.join("zone.json");
    std::fs::write(
        &manifest,
        serde_json::json!({
            "prefab_id": "prefab/zone",
            "structure_set": {
                "base": "zone",
                "size": [room.size[0], room.size[1], depth * 2],
                "part_max": 48,
                "grid": [1, 1, 2],
                "data_version": room.data_version,
                "generator": "crates/grammar",
                "parts": parts,
            },
        })
        .to_string(),
    )
    .unwrap();
    manifest
}

/// `audit` on a tile-set manifest audits every tile and returns ONE zone
/// verdict — the CLI path, not just the library function.
#[test]
fn audit_of_a_manifest_reports_the_zone() {
    let dir = tmp("tile-set");
    let manifest = stage_tile_set(&dir);

    let out = Command::new(bin())
        .arg("audit")
        .arg(&manifest)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{out:?}");
    let report: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("a machine-readable report");
    assert_eq!(report["verdict"], "pass");
    assert_eq!(
        report["tiles"].as_array().unwrap().len(),
        2,
        "the report names every tile the one verdict covers"
    );
    let room = fixtures::clean_room();
    assert_eq!(
        report["size"],
        serde_json::json!([room.size[0], room.size[1], room.size[2] * 2]),
        "the report sizes the zone, never a tile"
    );
}

/// ...and `audit` on ONE tile of that set is refused. A verdict over a fifth of
/// a zone that reads as a verdict over the zone is the failure mode this
/// command exists to prevent one layer up, so it must not be reachable by
/// pointing the command at the wrong file.
#[test]
fn audit_of_a_lone_tile_is_refused_and_names_the_manifest() {
    let dir = tmp("tile-alone");
    let manifest = stage_tile_set(&dir);

    let out = Command::new(bin())
        .arg("audit")
        .arg(dir.join("zone.x0y0z1.nbt"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("one tile of the zone"), "{stderr}");
    assert!(
        stderr.contains(manifest.to_str().unwrap()),
        "the refusal must name what to run instead: {stderr}"
    );
}
