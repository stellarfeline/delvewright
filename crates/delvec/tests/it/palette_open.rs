//! **Every vanilla block may be built with.** The admission audit admits every
//! block of the pinned registry outside the code-injection forbid list
//! (`DW0731`); no list of building blocks stands between a creator and a
//! vanilla block. Held at both doors a piece is admitted through — the
//! library call `delvec detail` makes and the `delvec prefab audit` command —
//! and at the block list a place generator plans with (`delvec grammar
//! blocks`). A command block is still refused.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use delvec::admit::audit::audit;
use delvec::admit::fixtures;
use delvec::admit::structure::{PaletteEntry, Structure};

/// The decoration and mechanism blocks a creator reaches for, each written in
/// its full default state (`DW0735` judges an omitted shape property).
const OPEN: &[&str] = &[
    "minecraft:dragon_egg",
    "minecraft:conduit",
    "minecraft:dragon_head",
    "minecraft:dragon_wall_head",
    "minecraft:lever",
    "minecraft:note_block",
    "minecraft:jukebox",
    "minecraft:piston",
    "minecraft:spawner",
    "minecraft:dispenser",
];

fn entry(id: &str) -> PaletteEntry {
    let registry = delvewright_dsl::blocks::BlockRegistry::v1_21_11();
    let state = registry
        .default_state(id)
        .unwrap_or_else(|| panic!("{id} is not in the pin"));
    let props: Vec<(&str, &str)> = state
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    PaletteEntry::with_props(id, &props)
}

/// The clean room with every block of `ids` standing on its floor.
fn piece(ids: &[&str]) -> Structure {
    let mut s = fixtures::clean_room();
    for (i, id) in ids.iter().enumerate() {
        let i = i as i32;
        s.set_cell([1 + i % 5, 1, 1 + i / 5], entry(id), None);
    }
    s
}

fn scratch(tag: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("palette-open-{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn delvec(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_delvec"))
        .args(args)
        .output()
        .expect("delvec runs")
}

fn said(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

/// The audit `delvec detail` runs admits a piece carrying every one of the
/// blocks, with no finding at all, and examines each.
#[test]
fn the_admission_audit_admits_every_vanilla_block_it_is_handed() {
    let (rep, diags) = audit("open", &piece(OPEN));
    assert!(rep.is_pass(), "{:?}", rep.findings);
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(rep.forbidden, 0);
    let examined = OPEN
        .iter()
        .filter(|id| rep.palette.iter().any(|b| b == *id))
        .count();
    assert_eq!(
        examined,
        OPEN.len(),
        "examined {examined} of {}",
        OPEN.len()
    );
}

/// `delvec prefab audit` admits the same piece, and refuses the same piece
/// with a command block in it under `DW0731`.
#[test]
fn prefab_audit_admits_them_and_still_refuses_a_command_block() {
    let dir = scratch("cli");
    let open = dir.join("open.nbt");
    std::fs::write(&open, piece(OPEN).write()).unwrap();
    let o = delvec(&["prefab", "audit", open.to_str().unwrap()]);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));

    let mut armed: Vec<&str> = OPEN.to_vec();
    armed.push("minecraft:command_block");
    let path = dir.join("armed.nbt");
    std::fs::write(&path, piece(&armed).write()).unwrap();
    let o = delvec(&["prefab", "audit", path.to_str().unwrap()]);
    let log = said(&o);
    assert_ne!(o.status.code(), Some(0), "{log}");
    assert!(
        log.contains("DW0731") && log.contains("minecraft:command_block"),
        "{log}"
    );
}

/// The list a place generator refuses by: every one of the blocks is
/// `admitted`, and `refused` is the code-injection forbid list alone.
#[test]
fn grammar_blocks_admits_them_and_refuses_only_the_forbid_list() {
    let o = delvec(&["grammar", "blocks"]);
    assert_eq!(o.status.code(), Some(0), "{}", said(&o));
    let blocks: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    let admitted: Vec<&str> = blocks["admitted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_str().unwrap())
        .collect();
    for id in OPEN {
        assert!(admitted.contains(id), "{id} is not admitted");
    }
    let refused: Vec<&str> = blocks["refused"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["block"].as_str().unwrap())
        .collect();
    assert!(!refused.is_empty(), "the forbid list is empty");
    for id in &refused {
        assert!(
            delvec::admit::audit::hard_forbidden_block(id),
            "{id} is refused but is no code-injection vector"
        );
    }
    assert!(refused.contains(&"minecraft:command_block"), "{refused:?}");
}
