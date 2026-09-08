//! **A document says nothing its own bytes deny** (`DW0888`) — the perturbation
//! that reds it, at every door that reads a prefab document together with its
//! `.nbt`.
//!
//! The finding this file is the general form of was found by hand: a library
//! round pointed `cave-shore`'s `connectors[0].local_pos` back at a cell the
//! regenerated structure fills with andesite, after which `delvec prefab audit`
//! passed through **both** its file arm and its directory arm at exit 0 and
//! `delvec prefab seating --horizon ocean` still answered four pools of four
//! seatable. So the perturbation here is that one — a socket planted at a solid
//! cell — and it is run at every door rather than at the one where the bug was
//! found.
//!
//! **The entry-point census is part of the deliverable.** Which commands read a
//! prefab document and its bytes together is discovered from `PrefabCommand`'s
//! own clap tree, not from a list somebody remembered, and every discovered
//! command is classified exactly once: a command added here and classified
//! nowhere is an ordinary red, including the one that would ship the hole. The
//! count is printed on every run.

use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

use clap::Subcommand;
use delvec::admit::cli::PrefabCommand;
use delvec::admit::fixtures;
use delvec::admit::socket::{SocketDecl, carve, opening_cells};
use delvec::admit::structure::{PaletteEntry, Structure};
use delvec::compiler::claims::DW_CLAIM_DENIED;

mod common;

fn delvec_bin() -> Proc {
    Proc::new(env!("CARGO_BIN_EXE_delvec"))
}

fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("byte-claims-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// A one-piece library whose document is true of its bytes: a room with a
/// carved socket, a walk plane read off the blocks, and one named place.
///
/// Written into `dir` and also into `dir/../honest`, which is what a
/// perturbation is restored from — a scratch copy rather than `git checkout`, so
/// the restore cannot quietly bring back anything else.
fn honest_library(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let mut s = fixtures::clean_room();
    let mut meta = delvewright_dsl::prefab::PrefabMeta::skeleton(
        "room",
        s.size,
        s.data_version,
        "crates/delvec/tests/prefab_byte_claims",
        delvewright_dsl::prefab::License {
            source: "original".into(),
            spdx: "GPL-3.0-or-later".into(),
            note: "Test fixture.".into(),
            provenance: "Synthesised by crates/delvec/tests/prefab_byte_claims.".into(),
            generated_by: None,
        },
    );
    carve(&mut s, &mut meta, &SocketDecl::new([3, 1, 0], "north")).unwrap();
    meta.anchors.insert(
        "anchor/stand".into(),
        delvewright_dsl::prefab::Anchor::point([3, 1, 3], "north"),
    );
    meta.walk_y = Some(1);
    std::fs::write(dir.join("room.nbt"), s.write()).unwrap();
    std::fs::write(dir.join("room.json"), meta.to_json()).unwrap();
    std::fs::write(
        dir.join("pools.json"),
        serde_json::json!({
            "pools": {
                "pool/room": {
                    "members": [{ "prefab": "prefab/room", "weight": 1, "role": "entry" }]
                }
            }
        })
        .to_string(),
    )
    .unwrap();
}

/// The perturbation: fill the socket's own opening back in with andesite, so the
/// connector the document declares points into solid rock. Nothing about the
/// document moves — this is a change to the BYTES the document claims to
/// describe, which is the direction the finding arrived in.
fn plant_a_socket_in_the_rock(dir: &Path) {
    let path = dir.join("room.nbt");
    let mut s = Structure::read(&std::fs::read(&path).unwrap()).unwrap();
    for cell in opening_cells([3, 1, 0], "north", [3, 3]) {
        s.set_cell(cell, PaletteEntry::simple("minecraft:andesite"), None);
    }
    std::fs::write(&path, s.write()).unwrap();
}

/// `delvec prefab audit <asset>` and `delvec prefab audit <dir>` over one
/// library, and the exit code beside the finding.
fn audit(target: &Path) -> Output {
    delvec_bin()
        .args(["prefab", "audit"])
        .arg(target)
        .output()
        .unwrap()
}

fn seating(dir: &Path, horizon: &str) -> Output {
    delvec_bin()
        .args(["prefab", "seating", "--horizon", horizon, "--prefabs"])
        .arg(dir)
        .output()
        .unwrap()
}

/// **Both audit arms, before and after.** The library is honest, both arms pass;
/// the socket is planted in the rock, and both arms red on `DW0888` — including
/// the directory arm, which is the one that had been carrying a check nothing
/// ever called.
#[test]
fn a_socket_planted_in_the_rock_reds_both_audit_arms() {
    let root = scratch("audit");
    let honest = root.join("honest");
    let under_test = root.join("library");
    honest_library(&honest);
    common::copy_dir_all(&honest, &under_test);

    for target in [under_test.join("room.nbt"), under_test.clone()] {
        let out = audit(&target);
        assert_eq!(
            out.status.code(),
            Some(0),
            "the honest library passes {}: {}",
            target.display(),
            text(&out)
        );
        assert!(
            text(&out).contains("byte-claim binding:"),
            "the binding is stated on the run that found nothing: {}",
            text(&out)
        );
    }

    plant_a_socket_in_the_rock(&under_test);

    for target in [under_test.join("room.nbt"), under_test.clone()] {
        let out = audit(&target);
        let log = text(&out);
        assert_ne!(
            out.status.code(),
            Some(0),
            "a socket in the rock must red {}: {log}",
            target.display()
        );
        assert!(log.contains(DW_CLAIM_DENIED.id()), "{log}");
        assert!(
            log.contains("not a way through"),
            "the message says what the bytes hold: {log}"
        );
        assert!(
            log.contains("connectors[].opening=1/1"),
            "the binding names the key and its denominator: {log}"
        );
    }

    // Restored from the scratch copy, not from git: both arms are green again,
    // so the red is the perturbation and nothing else in the tree.
    let _ = std::fs::remove_dir_all(&under_test);
    common::copy_dir_all(&honest, &under_test);
    assert_eq!(audit(&under_test).status.code(), Some(0));
}

/// **The seating command**, which the finding was measured against: it answered
/// seatable at exit 0 over a planted socket.
#[test]
fn the_seating_command_reds_on_a_planted_socket() {
    let root = scratch("seating");
    let honest = root.join("honest");
    let under_test = root.join("library");
    honest_library(&honest);
    common::copy_dir_all(&honest, &under_test);

    let out = seating(&under_test, "void");
    assert_eq!(
        out.status.code(),
        Some(0),
        "the honest library is seatable: {}",
        text(&out)
    );
    assert!(text(&out).contains("byte-claim binding:"), "{}", text(&out));

    plant_a_socket_in_the_rock(&under_test);
    let out = seating(&under_test, "void");
    let log = text(&out);
    assert_ne!(out.status.code(), Some(0), "{log}");
    assert!(log.contains(DW_CLAIM_DENIED.id()), "{log}");
}

/// **Every declaration of the class is bound, not only the one that was
/// noticed.** Each perturbation is a different key, and each of them reds.
#[test]
fn every_key_of_the_class_is_held_to_the_bytes() {
    let root = scratch("keys");
    let honest = root.join("honest");
    honest_library(&honest);

    // (what to change in the document, the key the binding must blame)
    let edits: &[(&str, fn(&mut serde_json::Value))] = &[
        ("walk_y=1/1", |doc| doc["walk_y"] = serde_json::json!(9)),
        ("structure.data_version=1/1", |doc| {
            doc["structure"]["data_version"] = serde_json::json!(1)
        }),
        ("anchors.*.pos=1/1", |doc| {
            doc["anchors"]["anchor/stand"]["pos"] = serde_json::json!([99, 1, 1])
        }),
        ("anchors.*.dispenser=1/1", |doc| {
            doc["anchors"]["anchor/stand"]["dispenser"] = serde_json::json!([3, 1, 3])
        }),
        ("anchors.*.trigger_block=1/1", |doc| {
            doc["anchors"]["anchor/stand"]["trigger_block"] =
                serde_json::json!("minecraft:stone_pressure_plate")
        }),
        ("connectors[].socket=1/1", |doc| {
            doc["connectors"][0]["target"] = serde_json::json!("cave:socket")
        }),
        ("jigsaw-declared=1/1", |doc| {
            // Moved one cell along the same wall: still an opening, and now
            // nothing declares the marker the piece really authors.
            doc["connectors"][0]["local_pos"] = serde_json::json!([4, 1, 0])
        }),
    ];

    let mut bound = 0usize;
    for (i, (expected, edit)) in edits.iter().enumerate() {
        let dir = root.join(format!("case-{i}"));
        common::copy_dir_all(&honest, &dir);
        let path = dir.join("room.json");
        let mut doc: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        edit(&mut doc);
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap() + "\n").unwrap();
        let out = audit(&dir);
        let log = text(&out);
        assert_ne!(out.status.code(), Some(0), "`{expected}` must red: {log}");
        assert!(log.contains(DW_CLAIM_DENIED.id()), "`{expected}`: {log}");
        assert!(
            log.contains(expected),
            "the binding must blame `{expected}`: {log}"
        );
        bound += 1;
    }
    assert_eq!(bound, edits.len());
    println!("byte-claim perturbation binding: {bound} key(s) of the class perturbed, {bound} red");
}

// ---------------------------------------------------------------------------
// The entry-point census
// ---------------------------------------------------------------------------

/// A `delvec prefab` command that reads a prefab document together with its
/// bytes, and therefore owes `DW0888`.
const READS_DOCUMENT_AND_BYTES: &[(&[&str], &str)] = &[
    (
        &["audit"],
        "one template, one tile-set manifest or a whole library directory — the admission event, \
         where a library's integrity lives",
    ),
    (
        &["seating"],
        "the whole library the global `--prefabs` points at, read against a horizon base",
    ),
];

/// A command that does not, each with the reason, printed on every run. The
/// polarity is deliberate: a list of inclusions fails silently when it misses a
/// site, a list of exclusions fails loudly.
const DOES_NOT: &[(&[&str], &str)] = &[
    (
        &["socket"],
        "WRITES the pair — it carves the opening and appends the matching connector in one act, so \
         the two cannot disagree at the moment they are made",
    ),
    (
        &["anchor"],
        "writes one named place into the document; the claim it makes is judged at `audit` and at \
         validation, where the bytes are read as a whole",
    ),
    (
        &["planes"],
        "MEASURES `walk_y`/`waterline_y` off the bytes and writes what it measured, so it cannot \
         produce a declaration the bytes deny",
    ),
    (
        &["lighting"],
        "measures light, which is a probe of a placed piece rather than a claim about the \
         template's own blocks",
    ),
    (
        &["resolve-jigsaw"],
        "rewrites foreign worldgen markers to their `final_state` at import, before a document \
         exists to make a claim",
    ),
    (
        &["gallery"],
        "builds a browse world out of candidate pieces; it makes no claim about any of them",
    ),
    (
        &["catalog", "validate"],
        "takes catalog cards — licence and schema text, never structure bytes",
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
    walk(
        &PrefabCommand::augment_subcommands(clap::Command::new("prefab")),
        &[],
        &mut out,
    );
    out.sort();
    out
}

/// **Every entry point is classified, and the set is discovered rather than
/// listed.** A command added to `delvec prefab` and classified nowhere is a red
/// here, which is what stops the rule reaching four doors of five again.
#[test]
fn every_prefab_entry_point_is_classified_and_the_census_is_stated() {
    let mut classified: Vec<Vec<String>> = READS_DOCUMENT_AND_BYTES
        .iter()
        .chain(DOES_NOT)
        .map(|(path, _)| path.iter().map(|s| (*s).to_string()).collect())
        .collect();
    classified.sort();
    let discovered = discovered_commands();
    assert_eq!(
        discovered, classified,
        "every `delvec prefab` command is classified exactly once — discovered {discovered:?}, \
         classified {classified:?}"
    );
    assert!(
        !READS_DOCUMENT_AND_BYTES.is_empty(),
        "a census that binds nothing is the unbound vacuity mode"
    );
    // A COMMAND is not a door: `audit` is three of them, and the directory arm
    // is the one that was carrying a check nothing ever called. The census
    // counts arms, and every one of them is exercised by a test in this file.
    let arms = [
        "`prefab audit <asset.nbt>` — one template, at the admission event",
        "`prefab audit <manifest.json>` — a tiled zone, measured as one assembled building",
        "`prefab audit <library dir>` — the whole library at once",
        "`prefab seating --horizon <base>` — the library the global `--prefabs` points at",
        "`main::validate_loaded` -> `compiler::seating::check` — the one validation funnel every \
         subcommand goes through, `build` included",
    ];
    println!(
        "byte-claim entry-point census: {total} `delvec prefab` command(s) discovered, {reads} \
         read a document with its bytes, {other} do not; {n} entry points bound in all, counting \
         each arm of `audit` separately and the compiler's own validation funnel:",
        total = discovered.len(),
        reads = READS_DOCUMENT_AND_BYTES.len(),
        other = DOES_NOT.len(),
        n = arms.len(),
    );
    for arm in arms {
        println!("  door: {arm}");
    }
    for (path, why) in DOES_NOT {
        println!("  not a door: `{}` — {why}", path.join(" "));
    }
}

/// **The tile-set arm**: a zone that ships as several `.nbt` files is one
/// building, and its manifest IS its prefab document — so the rule reads it at
/// zone scale exactly as it reads a single template.
#[test]
fn a_tiled_zone_is_judged_through_its_manifest() {
    let root = scratch("tiled");
    let dir = root.join("prefabs");
    common::write_tiled_zone(
        &dir,
        "tiled-corridor",
        serde_json::json!({ "anchor/stand": { "pos": [4, 1, 4], "facing": "north" } }),
        &[],
    );
    let manifest = dir.join("tiled-corridor.json");
    let out = audit(&manifest);
    assert_eq!(
        out.status.code(),
        Some(0),
        "an honest zone passes: {}",
        text(&out)
    );
    assert!(
        text(&out).contains("byte-claim binding:"),
        "the arm states its binding: {}",
        text(&out)
    );

    // A named place outside the ASSEMBLED zone — a claim only the whole
    // building can answer, which is why the arm exists.
    let mut doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();
    doc["anchors"]["anchor/stand"]["pos"] = serde_json::json!([4, 1, 9999]);
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&doc).unwrap() + "\n",
    )
    .unwrap();
    let out = audit(&manifest);
    let log = text(&out);
    assert_ne!(out.status.code(), Some(0), "{log}");
    assert!(log.contains(DW_CLAIM_DENIED.id()), "{log}");
    assert!(log.contains("anchors.*.pos=1/1"), "{log}");
}

/// **The validation funnel**, which is the entry point no `delvec prefab`
/// command is: a campaign that seats the piece is refused before a block is
/// placed, so `build` cannot reach a datapack past it.
#[test]
fn a_campaign_that_seats_the_piece_is_refused_at_validation() {
    let root = scratch("validate");
    let dir = root.join("library");
    honest_library(&dir);
    plant_a_socket_in_the_rock(&dir);

    // The shipped hello-world campaign, with its one area rebound to this piece.
    let camp = root.join("campaign");
    common::copy_dir_all(&common::hello_world_dir(), &camp);
    let world_path = camp.join("world.json");
    let mut world: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&world_path).unwrap()).unwrap();
    for area in world["content"]["areas"].as_array_mut().unwrap() {
        area["prefab"] = serde_json::json!("prefab/room");
    }
    std::fs::write(
        &world_path,
        serde_json::to_string_pretty(&world).unwrap() + "\n",
    )
    .unwrap();

    let out = delvec_bin()
        .args(["validate"])
        .arg(&camp)
        .arg("--prefabs")
        .arg(&dir)
        .output()
        .unwrap();
    let log = text(&out);
    assert_ne!(out.status.code(), Some(0), "{log}");
    assert!(log.contains(DW_CLAIM_DENIED.id()), "{log}");
    assert!(
        !log.contains("place template"),
        "refused before anything is placed: {log}"
    );
}
