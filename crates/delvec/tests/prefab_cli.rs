//! End-to-end CLI: exit codes + on-disk effects of the admission subcommands.

use std::path::PathBuf;
use std::process::Command;

use delvewright_admit::fixtures;
use delvewright_admit::meta::PrefabMeta;

/// `delvec prefab …`: the one binary, entered at the prefab-admission surface.
fn prefab() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_delvec"));
    cmd.arg("prefab");
    cmd
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delvec prefab-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn audit_exit_codes_and_report() {
    let dir = tmp("audit");
    let clean = dir.join("clean.nbt");
    let cb = dir.join("cb.nbt");
    std::fs::write(&clean, fixtures::clean_room().write()).unwrap();
    std::fs::write(&cb, fixtures::command_block_piece().write()).unwrap();

    // clean -> exit 0, report verdict "pass".
    let out = prefab().arg("audit").arg(&clean).output().unwrap();
    assert!(out.status.success());
    let report = String::from_utf8(out.stdout).unwrap();
    assert!(report.contains("\"verdict\": \"pass\""), "{report}");

    // command block -> exit 1, report verdict "fail".
    let out = prefab().arg("audit").arg(&cb).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let report = String::from_utf8(out.stdout).unwrap();
    assert!(report.contains("\"verdict\": \"fail\""), "{report}");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn socket_and_lighting_write_metadata() {
    let dir = tmp("chain");
    let nbt = dir.join("piece.nbt");
    std::fs::write(&nbt, fixtures::clean_room().write()).unwrap();

    // carve a north socket (auto-creates a skeleton metadata).
    let out = prefab()
        .args(["socket"])
        .arg(&nbt)
        .args(["--pos", "3,1,0", "--facing", "north"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // annotate an anchor.
    let out = prefab()
        .args(["anchor"])
        .arg(&nbt)
        .args([
            "--name",
            "anchor/npc-stand",
            "--pos",
            "3,1,3",
            "--facing",
            "north",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    // probe + write lighting.
    let out = prefab()
        .args(["lighting"])
        .arg(&nbt)
        .arg("--write")
        .output()
        .unwrap();
    assert!(out.status.success());

    // metadata now carries the socket, anchor, and lighting.
    let meta = PrefabMeta::beside_nbt(&nbt).unwrap().unwrap();
    assert_eq!(meta.connectors.len(), 1);
    assert!(meta.anchors.contains_key("anchor/npc-stand"));
    let lighting = meta.lighting.expect("--write records the probe");
    assert_eq!(
        lighting.profile,
        delvewright_admit::meta::LightingProfile::Lit
    );
    assert!(lighting.method.as_deref().unwrap().contains("static"));

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn gallery_and_curate_merge_end_to_end() {
    let dir = tmp("gallery");
    let candidates = dir.join("candidates");
    std::fs::create_dir_all(&candidates).unwrap();
    std::fs::write(
        candidates.join("gatehouse.nbt"),
        fixtures::clean_room().write(),
    )
    .unwrap();

    let out_dir = dir.join("out");
    let status = prefab()
        .args(["gallery"])
        .arg(&candidates)
        .args(["-o"])
        .arg(&out_dir)
        .args(["--id", "demo"])
        .status()
        .unwrap();
    assert!(status.success());
    let layout = out_dir.join("gallery-layout.json");
    assert!(layout.exists());
    assert!(out_dir.join("datapack/pack.mcmeta").exists());

    // a playtest log resolving a note to the piece (asset id = file stem here).
    let log = dir.join("server.log");
    std::fs::write(
        &log,
        "[12:00:20] [Server thread/INFO]: [c] [DelveNote] pos=[3,65,3] area=gatehouse quests= nearest_npc=none\n\
         [12:00:23] [Server thread/INFO]: <c> keep it\n",
    )
    .unwrap();

    // curate -> report.
    let report = dir.join("curation.json");
    let status = prefab()
        .args(["curate"])
        .arg(&log)
        .args(["--layout"])
        .arg(&layout)
        .args(["-o"])
        .arg(&report)
        .status()
        .unwrap();
    assert!(status.success());

    // a catalog card for the asset, then merge the curation into it.
    let catalog_dir = dir.join("catalog");
    std::fs::create_dir_all(&catalog_dir).unwrap();
    let card = r#"{
      "asset_id": "gatehouse",
      "description": "A small stone gatehouse used as a gallery fixture piece here.",
      "tags": { "era_style": "medieval", "condition": "intact", "scale_class": "small", "interior_exterior": "interior" },
      "style_fit": { "verdict": "borderline", "rationale": "placeholder" },
      "quality": 3,
      "license": { "spdx": "original", "source": "original" }
    }"#;
    std::fs::write(catalog_dir.join("gatehouse.json"), card).unwrap();

    let status = prefab()
        .args(["curate-merge"])
        .arg(&report)
        .args(["--catalog"])
        .arg(&catalog_dir)
        .status()
        .unwrap();
    assert!(status.success());

    let merged = delvewright_admit::catalog::CatalogCard::from_json(
        &std::fs::read_to_string(catalog_dir.join("gatehouse.json")).unwrap(),
    )
    .unwrap();
    let cur = merged.curation.unwrap();
    assert_eq!(cur.notes.len(), 1);
    assert_eq!(cur.notes[0].text, "keep it");

    std::fs::remove_dir_all(&dir).ok();
}
