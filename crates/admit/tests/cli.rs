//! End-to-end CLI: exit codes + on-disk effects of the admission subcommands.

use std::path::PathBuf;
use std::process::Command;

use delvewright_admit::fixtures;
use delvewright_admit::meta::{AnchorRole, PrefabMeta};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_delve-admit")
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delve-admit-{}-{name}", std::process::id()));
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
    let out = Command::new(bin())
        .arg("audit")
        .arg(&clean)
        .output()
        .unwrap();
    assert!(out.status.success());
    let report = String::from_utf8(out.stdout).unwrap();
    assert!(report.contains("\"verdict\": \"pass\""), "{report}");

    // command block -> exit 1, report verdict "fail".
    let out = Command::new(bin()).arg("audit").arg(&cb).output().unwrap();
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
    let out = Command::new(bin())
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
    let out = Command::new(bin())
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
    let out = Command::new(bin())
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
    let status = Command::new(bin())
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
    let status = Command::new(bin())
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

    let status = Command::new(bin())
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

// ---------------------------------------------------------------------------
// `anchor --role` — an anchor's role is written where the anchor is
// ---------------------------------------------------------------------------

/// **The gap this closes.** `anchor` could say where a place is and not what it
/// is for, so a piece admitted through this route could never declare an entry
/// point: every campaign built from new pieces was refused by `DW0345`, and the
/// only way to give one a role was to hand-edit the JSON the tool had written.
///
/// Five claims, one document: the role is written; an unknown term is refused
/// **where it is typed** rather than ridden through into the metadata; the role
/// survives an edit that says nothing about it (moving a cell is not a statement
/// that the piece stopped being the way in); `--no-role` removes it, which is the
/// remedy `DW0804` prescribes; and the two flags cannot both be given.
#[test]
fn anchor_writes_and_clears_the_role_and_refuses_a_term_it_does_not_know() {
    let dir = tmp("anchor-role");
    let nbt = dir.join("piece.nbt");
    std::fs::write(&nbt, fixtures::clean_room().write()).unwrap();

    let anchor = |args: &[&str]| {
        Command::new(bin())
            .arg("anchor")
            .arg(&nbt)
            .args(["--name", "anchor/arrival"])
            .args(args)
            .output()
            .unwrap()
    };
    let role_now = || -> Option<AnchorRole> {
        PrefabMeta::beside_nbt(&nbt).unwrap().unwrap().anchors["anchor/arrival"].role
    };

    // Written.
    let out = anchor(&["--pos", "3,1,3", "--facing", "north", "--role", "entry"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(role_now(), Some(AnchorRole::Entry));

    // An unknown term is refused at exit 2, naming both the term and the
    // vocabulary, and nothing is written: the document still says what it said.
    let out = anchor(&["--pos", "3,1,3", "--role", "dispenser"]);
    assert_eq!(
        out.status.code(),
        Some(2),
        "an unknown role is an input error"
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("dispenser") && stderr.contains("`entry`"),
        "{stderr}"
    );
    assert_eq!(
        role_now(),
        Some(AnchorRole::Entry),
        "a refused role must not have moved the document"
    );

    // Silent about the role → the role stays. This is the case that would
    // otherwise delete a piece's entry point every time somebody nudged a cell.
    let out = anchor(&["--pos", "4,1,4", "--facing", "south"]);
    assert!(out.status.success());
    assert_eq!(role_now(), Some(AnchorRole::Entry));

    // ...and `--no-role` says it has none.
    let out = anchor(&["--pos", "4,1,4", "--no-role"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(role_now(), None);

    // The two are contradictory and the parser says so rather than picking one.
    let out = anchor(&["--pos", "4,1,4", "--role", "entry", "--no-role"]);
    assert_eq!(out.status.code(), Some(2));

    std::fs::remove_dir_all(&dir).ok();
}
