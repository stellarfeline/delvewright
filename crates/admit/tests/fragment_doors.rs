//! Which commands owe a lone tile a refusal — and does **every** one of them?
//!
//! A zone past the 48-per-axis structure-template cap ships as several `.nbt`
//! tiles plus one manifest. Every command that opens a piece an author named will
//! be handed one of those tiles some day, and every one of them would then answer
//! confidently about a building nobody has: `audit` returns `pass` over a fifth of
//! a zone, `lighting` measures a fifth of a building, `socket` edits one tile of a
//! set out of step with the rest. The refusal is `DW0739`, and it lives once, in
//! `delvewright_dsl::split::{tile_evidence, fragment_refusal}`.
//!
//! `tests/cli_errors.rs` already proves that refusal is *correct*. This file
//! answers the different question — **at which commands is it obliged?** — and it
//! answers it by ENUMERATING the parser rather than by listing doors. A list of
//! doors is what produced the defect: the guard was written for the door in front
//! of the author, the next command to open a piece never learned about it, and
//! nothing anywhere asked what the set was. So the set is discovered from
//! [`Cli::command()`], every discovered command is classified exactly once, and a
//! command added and classified nowhere is an ordinary red — including the one
//! that would have shipped the hole.
//!
//! Two obligations, and the second is what keeps the exemption honest:
//!
//! 1. **A piece door handed a lone tile refuses with `DW0739`**, both when the
//!    tile sits beside its manifest and when it has been copied away from it.
//! 2. **NO command — door or exempt — succeeds when handed a lone tile.** An
//!    exemption may downgrade *which* diagnostic is required; it can never hide an
//!    exit 0. That is the property the defect cannot supply: a command that does
//!    not read structure bytes cannot digest a tile and report success, so writing
//!    a new door into [`NOT_PIECE_DOORS`] to quiet this test fails on the very
//!    invocation the entry claims is irrelevant.
//!
//! The binding count is printed on every run: a check that matched nothing is a
//! finding, not a pass (CLAUDE.md).

use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

use clap::CommandFactory;
use delvewright_admit::cli::Cli;
use delvewright_admit::fixtures;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_delve-admit")
}

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("delve-admit-doors-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// What a door is handed: the piece itself, or a directory of pieces.
#[derive(Clone, Copy, PartialEq)]
enum Handed {
    /// One `.nbt` path.
    Nbt,
    /// A directory that happens to hold a tile set — the door nobody points at
    /// deliberately, and the one that walks `*.nbt` and plinths five slices of
    /// one building.
    Dir,
}

/// A command that opens a piece an author named, and the arguments it needs
/// besides that piece.
struct Door {
    /// The subcommand path, as typed.
    path: &'static [&'static str],
    /// Everything else the command requires, with values that PARSE — an
    /// argument rejected before the piece is opened would green this test on a
    /// usage error rather than on a refusal.
    extra: &'static [&'static str],
    handed: Handed,
}

/// Every command that reads a piece. Discovered set minus this is
/// [`NOT_PIECE_DOORS`], checked in both directions below.
const PIECE_DOORS: &[Door] = &[
    Door {
        path: &["audit"],
        extra: &[],
        handed: Handed::Nbt,
    },
    Door {
        path: &["socket"],
        extra: &["--pos", "3,1,0", "--facing", "north"],
        handed: Handed::Nbt,
    },
    Door {
        path: &["resolve-jigsaw"],
        extra: &[],
        handed: Handed::Nbt,
    },
    Door {
        path: &["anchor"],
        extra: &["--name", "anchor/nave", "--pos", "1,1,1"],
        handed: Handed::Nbt,
    },
    Door {
        path: &["lighting"],
        extra: &[],
        handed: Handed::Nbt,
    },
    Door {
        path: &["gallery"],
        extra: &[],
        handed: Handed::Dir,
    },
];

/// Commands that do not open a piece, each with the reason, PRINTED on every
/// run. The polarity is deliberate and is the same one `check-structure-emitters`
/// states: a list of inclusions fails silently when it misses a site, a list of
/// exclusions fails loudly. And an entry here still owes obligation 2 — it may
/// not exit 0 on a tile — so the list cannot be used to smuggle a door through.
const NOT_PIECE_DOORS: &[(&[&str], &str)] = &[
    (
        &["catalog", "validate"],
        "takes catalog cards (`catalog/<id>.json`) — licence and schema text, never structure \
         bytes",
    ),
    (
        &["curate"],
        "takes a gallery playtest's server log and that gallery's layout JSON",
    ),
    (
        &["curate-merge"],
        "takes a curation report and a catalog directory, and edits cards",
    ),
];

/// Every leaf command the parser has, as the words an author types.
///
/// `help` is clap's own and reads no input, so it is dropped here rather than
/// exempted — an entry in [`NOT_PIECE_DOORS`] should be a decision about this
/// tool's surface, not about the parser's furniture.
fn discovered_commands() -> Vec<Vec<String>> {
    fn walk(cmd: &clap::Command, prefix: &[String], out: &mut Vec<Vec<String>>) {
        for sub in cmd.get_subcommands() {
            let name = sub.get_name().to_string();
            if name == "help" {
                continue;
            }
            let mut path = prefix.to_vec();
            path.push(name);
            if sub.get_subcommands().next().is_some() {
                walk(sub, &path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&Cli::command(), &[], &mut out);
    out.sort();
    out
}

/// Stage a two-tile zone, and a copy of one tile with nothing beside it.
///
/// Returns `(dir, tile beside its manifest, the same tile copied away)`.
fn stage(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let dir = tmp(name);
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
    std::fs::write(
        dir.join("zone.json"),
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
            "connectors": [],
            "lighting": { "profile": "unmeasured" },
        })
        .to_string(),
    )
    .unwrap();

    let beside = dir.join("zone.x0y0z1.nbt");
    let elsewhere = dir.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    let orphan = elsewhere.join("zone.x0y0z1.nbt");
    std::fs::copy(&beside, &orphan).unwrap();
    assert!(
        !elsewhere.join("zone.json").exists(),
        "nothing beside the copy says what it is"
    );
    (dir, beside, orphan)
}

/// Run one command with `piece` in its input position.
fn run(path: &[&str], extra: &[&str], piece: &Path, out_dir: &Path) -> Output {
    let mut cmd = Proc::new(bin());
    cmd.args(path).arg(piece).args(extra);
    // The two commands that write a tree need somewhere to write it. Handing
    // them a real output path is what makes an exit 0 mean "it answered", rather
    // than "it could not find its own arguments".
    match path {
        ["gallery"] => {
            cmd.arg("-o").arg(out_dir.join("gallery"));
        }
        ["curate"] => {
            cmd.arg("--layout").arg(piece);
        }
        ["curate-merge"] => {
            cmd.arg("--catalog").arg(out_dir);
        }
        _ => {}
    }
    cmd.output().unwrap()
}

/// Obligation 1: every piece door refuses a lone tile by name, wherever the tile
/// is.
#[test]
fn every_piece_door_refuses_a_lone_tile() {
    let (dir, beside, orphan) = stage("refuses");
    let mut bound = 0usize;
    for door in PIECE_DOORS {
        // Beside its manifest, the refusal can say which zone this is a tile of
        // and what to run instead.
        let handed_beside: PathBuf = match door.handed {
            Handed::Nbt => beside.clone(),
            Handed::Dir => dir.clone(),
        };
        let out = run(door.path, door.extra, &handed_beside, &dir);
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(
            out.status.code(),
            Some(2),
            "{:?} handed a tile beside its manifest: {stderr}",
            door.path
        );
        assert!(stderr.contains("DW0739"), "{:?}: {stderr}", door.path);
        assert!(
            stderr.contains("zone.json"),
            "{:?}: the refusal must name the manifest to pass instead: {stderr}",
            door.path
        );

        // Copied away from it, the verdict is unchanged — the evidence is the
        // tile's own name, so a `cp` cannot launder a fragment into a prefab.
        let handed_orphan: PathBuf = match door.handed {
            Handed::Nbt => orphan.clone(),
            Handed::Dir => dir.join("elsewhere"),
        };
        let out = run(door.path, door.extra, &handed_orphan, &dir);
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(
            out.status.code(),
            Some(2),
            "{:?} handed a detached tile: {stderr}",
            door.path
        );
        assert!(stderr.contains("DW0739"), "{:?}: {stderr}", door.path);
        assert!(
            stderr.contains("separated from its set"),
            "{:?}: {stderr}",
            door.path
        );
        bound += 1;
    }
    assert!(bound > 0, "a gate that examined no doors is a finding");
    eprintln!("fragment-door binding: {bound} piece door(s) refused a lone tile, twice each");
}

/// Obligation 2: **no** command succeeds when handed a lone tile.
///
/// This is the one an exemption cannot escape. `NOT_PIECE_DOORS` says a command
/// does not read pieces; the way that claim is checked is by handing it a piece
/// and requiring that it does not report success. A door written into that list
/// to quiet the test above fails here instead, because a real door digests the
/// tile and exits 0 — which is exactly the defect.
#[test]
fn no_command_succeeds_on_a_lone_tile() {
    let (dir, _beside, orphan) = stage("no-success");
    let mut bound = 0usize;
    for (path, reason) in NOT_PIECE_DOORS {
        let out = run(path, &[], &orphan, &dir);
        let code = out.status.code();
        assert_ne!(
            code,
            Some(0),
            "{path:?} is exempt because it {reason}, yet it accepted one tile of a zone and \
             reported success — it is a piece door, and it owes DW0739"
        );
        eprintln!(
            "exempt: {path:?} — {reason} (handed a tile: exit {})",
            code.map(|c| c.to_string()).unwrap_or("signal".into())
        );
        bound += 1;
    }
    // ...and the doors themselves, for the same reason, at the same strength.
    for door in PIECE_DOORS {
        let handed: PathBuf = match door.handed {
            Handed::Nbt => orphan.clone(),
            Handed::Dir => dir.join("elsewhere"),
        };
        let out = run(door.path, door.extra, &handed, &dir);
        assert_ne!(out.status.code(), Some(0), "{:?}", door.path);
        bound += 1;
    }
    eprintln!("no-success binding: {bound} command(s) examined, 0 accepted a fragment");
}

/// The classification is total, in both directions.
///
/// A new command that opens a piece is the way this defect returns, and the only
/// thing that can catch it is a set nobody wrote by hand. So: every command the
/// parser has is classified exactly once, and every classified command exists.
#[test]
fn every_command_is_classified_exactly_once() {
    let discovered = discovered_commands();
    assert!(
        !discovered.is_empty(),
        "the parser reported no commands — this gate would bind to nothing"
    );

    let mut classified: Vec<Vec<String>> = Vec::new();
    for door in PIECE_DOORS {
        classified.push(door.path.iter().map(|s| s.to_string()).collect());
    }
    for (path, _) in NOT_PIECE_DOORS {
        classified.push(path.iter().map(|s| s.to_string()).collect());
    }
    classified.sort();

    let mut duplicated = classified.clone();
    duplicated.dedup();
    assert_eq!(
        duplicated.len(),
        classified.len(),
        "a command is classified twice: {classified:?}"
    );

    let unclassified: Vec<&Vec<String>> = discovered
        .iter()
        .filter(|c| !classified.contains(c))
        .collect();
    assert!(
        unclassified.is_empty(),
        "these commands are classified nowhere: {unclassified:?}. A command that opens a piece an \
         author named goes in PIECE_DOORS and owes a lone tile DW0739; one that does not goes in \
         NOT_PIECE_DOORS with the reason. Deciding is the point — the guard reached two doors of \
         three because nothing ever asked what the set was."
    );

    let vanished: Vec<&Vec<String>> = classified
        .iter()
        .filter(|c| !discovered.contains(c))
        .collect();
    assert!(
        vanished.is_empty(),
        "these classifications name commands that no longer exist: {vanished:?}"
    );

    eprintln!(
        "classification binding: {} command(s) discovered, {} piece door(s), {} exempt",
        discovered.len(),
        PIECE_DOORS.len(),
        NOT_PIECE_DOORS.len()
    );
}
