//! CLI-level error-path tests: input, tooling, dark-lighting-advisory, and
//! gallery-write diagnostics. Complements `tests/cli.rs` (happy-path chains)
//! and `tests/audit.rs`/`tests/catalog.rs` (already cover `DW0730`/`DW0731`/
//! `DW0740`/`DW0741` at the library level).

use std::path::PathBuf;
use std::process::Command;

use delvewright_admit::fixtures;

/// `delvec prefab …`: the one binary, entered at the prefab-admission surface.
fn prefab() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_delvec"));
    cmd.arg("prefab");
    cmd
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delvec prefab-err-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `DW0732`: an unreadable input file — `audit` on a path that does not exist.
#[test]
fn audit_missing_file_is_dw0732_exit2() {
    let dir = tmp("missing-input");
    let out = prefab()
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

    let out = prefab()
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

    let out = prefab().args(["lighting"]).arg(&nbt).output().unwrap();
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

    let out = prefab()
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
            "anchors": { "anchor/nave": { "pos": [3, 1, 3], "facing": "north" } },
            "connectors": [],
            "lighting": { "profile": "unmeasured" },
            "license": {
                "source": "original",
                "spdx": "GPL-3.0-or-later",
                "note": "n",
                "provenance": "Generated deterministically by crates/grammar",
                "generated_by": {
                    "generator": "grammar",
                    "program": "zone",
                    "program_hash": "sha256:00",
                    "seed": 1
                }
            }
        })
        .to_string(),
    )
    .unwrap();
    manifest
}

/// `lighting` on a tile-set manifest probes the WHOLE zone.
///
/// The documented procedure (`prefab-procedure.md` §7) is `audit`, `socket`,
/// `lighting`, `audit` — and for every zone past the 48-per-axis cap the third
/// step used to be unreachable: handed the manifest, `lighting` tried to gunzip
/// JSON and died at `DW0732`. A building bigger than 48 blocks could not have
/// its light measured at all.
#[test]
fn lighting_of_a_manifest_probes_the_whole_zone() {
    let dir = tmp("lighting-manifest");
    let manifest = stage_tile_set(&dir);

    let out = prefab().args(["lighting"]).arg(&manifest).output().unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["files"], 2, "both tiles were read");
    let room = fixtures::clean_room();
    assert_eq!(
        report["size"],
        serde_json::json!([room.size[0], room.size[1], room.size[2] * 2]),
        "the probe sizes the zone, never a tile"
    );
    // Every artifact states its binding, and the binding is a subset chain.
    let b = &report["binding"];
    let (standable, measured) = (
        b["standable_cells"].as_u64().unwrap(),
        b["measured_cells"].as_u64().unwrap(),
    );
    assert!(measured > 0, "a zero binding is a finding, not a pass");
    assert!(measured <= standable, "{b}");
    // ...and the sky it assumed, because a light level without one is unreadable.
    let sky = &report["assumed_sky"];
    let (profile_sky, daylight) = (
        sky["profile_taken_at"].as_i64().unwrap(),
        sky["daylight"].as_i64().unwrap(),
    );
    assert!(
        profile_sky < daylight,
        "the profile is taken at the darker of the two skies: {sky}"
    );
    assert!(
        report["min_light_daylight"].as_i64().unwrap()
            >= report["measured_min_light"].as_i64().unwrap(),
        "more sky is never less light: {report}"
    );
}

/// `--write` on a manifest edits the ZONE's own metadata, and leaves the rest of
/// the document exactly as it found it.
///
/// The provenance row is the point. Before this, `--write` on a tiled zone was
/// unreachable at all, and the tile it was pointed at instead got a manufactured
/// skeleton claiming `spdx: UNKNOWN`.
#[test]
fn lighting_write_on_a_manifest_keeps_the_zones_provenance() {
    let dir = tmp("lighting-manifest-write");
    let manifest = stage_tile_set(&dir);
    let before: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();

    let out = prefab()
        .args(["lighting"])
        .arg(&manifest)
        .arg("--write")
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let after: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
    assert_eq!(after["license"], before["license"], "provenance survives");
    assert_eq!(after["structure_set"], before["structure_set"]);
    assert_eq!(after["anchors"], before["anchors"]);
    assert_eq!(after["lighting"]["profile"], "lit");
    let method = after["lighting"]["method"].as_str().unwrap();
    assert!(
        method.contains("floor cell(s) reachable on foot"),
        "the method states the binding the measurement was taken over: {after}"
    );
    assert!(
        method.contains("effective sky") && method.contains("full daylight"),
        "...and the sky it was taken at, twice over: {after}"
    );
    // ...and no per-tile metadata was manufactured beside it.
    assert!(!dir.join("zone.x0y0z0.json").exists());
}

/// `DW0739`: a whole-piece command handed ONE TILE is refused — and the refusal
/// survives the tile being copied away from its manifest.
///
/// This is the general form. The old guard asked "is there a manifest beside
/// this file naming it", so `cp tile.nbt elsewhere/` produced a fragment every
/// tool accepted as a whole prefab: a guard that depends on a neighbouring file
/// is not a property of the artifact. The tile's NAME is — `part_filename`
/// writes it, and a copy, a move and an upload all carry it.
#[test]
fn a_detached_tile_is_still_refused_at_every_door() {
    let dir = tmp("detached-tile");
    stage_tile_set(&dir);
    let elsewhere = dir.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let orphan = elsewhere.join("zone.x0y0z1.nbt");
    std::fs::copy(dir.join("zone.x0y0z1.nbt"), &orphan).unwrap();
    assert!(
        !elsewhere.join("zone.json").exists(),
        "nothing beside it says what it is"
    );

    for args in [
        vec!["audit".to_string()],
        vec!["lighting".to_string()],
        vec!["resolve-jigsaw".to_string()],
    ] {
        let out = prefab().args(&args).arg(&orphan).output().unwrap();
        assert_eq!(out.status.code(), Some(2), "{args:?}: {out:?}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains("DW0739"), "{args:?}: {stderr}");
        assert!(stderr.contains("separated from its set"), "{stderr}");
    }

    // ...and the editing commands, which would corrupt the set rather than
    // merely misreport it.
    let out = prefab()
        .args(["socket"])
        .arg(&orphan)
        .args(["--pos", "3,1,0", "--facing", "north"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("DW0739"));

    // ...and the door nobody points at deliberately: a gallery over a directory
    // that holds tiles would put slices of one building on five plinths.
    let out = prefab()
        .args(["gallery"])
        .arg(&elsewhere)
        .args(["-o"])
        .arg(dir.join("gallery-out"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("DW0739"));
}

/// `DW0753`: `--write` with no metadata to write into REFUSES, and writes
/// nothing.
///
/// It used to manufacture one: `source: unknown`, `spdx: UNKNOWN`, no
/// `generated_by` row and `anchors: {}` — a document asserting that nothing is
/// known about the asset, written silently and indistinguishable afterwards
/// from a real admission record. A tool that cannot establish where something
/// came from must refuse, never invent.
#[test]
fn lighting_write_without_metadata_is_dw0753_and_writes_nothing() {
    let dir = tmp("no-provenance");
    let nbt = dir.join("piece.nbt");
    std::fs::write(&nbt, fixtures::clean_room().write()).unwrap();

    let out = prefab()
        .args(["lighting"])
        .arg(&nbt)
        .arg("--write")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DW0753"), "{stderr}");
    assert!(
        !dir.join("piece.json").exists(),
        "no metadata may be invented"
    );
    // The measurement itself is still printed — the refusal is about writing.
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["profile"], "lit");
}

/// `DW0752`: a probe that bound to ZERO cells is a finding, never a pass.
///
/// This is the sixth-mode question asked of this gate itself: what does it
/// demand, and could the defect supply it? A pitch-black sealed crypt binds no
/// cells — so if "nothing to measure" were a success, the darkest possible piece
/// would be the one that passed most quietly.
#[test]
fn a_sealed_piece_binds_zero_cells_and_is_dw0752() {
    let dir = tmp("sealed");
    let nbt = dir.join("sealed.nbt");
    std::fs::write(&nbt, fixtures::sealed_room().write()).unwrap();

    let out = prefab().args(["lighting"]).arg(&nbt).output().unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("DW0752"), "{stderr}");
    assert!(stderr.contains("no ground-level entrance"), "{stderr}");
    let report: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["profile"], "unbound");
    assert_eq!(report["binding"]["measured_cells"], 0);
    assert!(
        report["binding"]["standable_cells"].as_u64().unwrap() > 0,
        "there IS floor in it — what is missing is a way in: {report}"
    );
}

/// `audit` on a tile-set manifest audits every tile and returns ONE zone
/// verdict — the CLI path, not just the library function.
#[test]
fn audit_of_a_manifest_reports_the_zone() {
    let dir = tmp("tile-set");
    let manifest = stage_tile_set(&dir);

    let out = prefab().arg("audit").arg(&manifest).output().unwrap();
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

    let out = prefab()
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
