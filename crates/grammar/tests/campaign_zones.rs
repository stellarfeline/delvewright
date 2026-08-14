//! **The campaign's own zone programs**, judged from the files rather than from
//! the engine's copies of them.
//!
//! `zones.rs` sweeps `library::bell::*` — Rust functions in this repo. Those are
//! transitional: ADR-0018 makes the IR the artifact, and the campaign's
//! `design/programs/*.json` is where a zone actually lives and is edited. The
//! two have already diverged (Z2 was authored forward in the campaign after it
//! was exported), so a proof about the Rust copy is not a proof about the thing
//! that ships. This file binds to the files.
//!
//! It reads them through the `campaigns/` symlink the repo uses everywhere else
//! (CI materialises it with `./.github/actions/checkout-content`). A missing
//! checkout is a **failure**, never a skip: a test that quietly passes when its
//! subject is absent is the vacuity this whole round is about.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use delvewright_grammar::ir::Program;
use delvewright_grammar::{Box3, ExpandOptions, expand};

/// One zone as its campaign declares it in `design/programs/zones.json`.
#[derive(serde::Deserialize)]
struct ZoneEntry {
    id: String,
    program: String,
    region: [u32; 3],
    seed: u64,
}

#[derive(serde::Deserialize)]
struct ZoneManifest {
    zones: Vec<ZoneEntry>,
}

fn content_campaigns() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../campaigns/campaigns");
    assert!(
        root.is_dir(),
        "{} is not a directory — the content checkout is missing. This test is about the \
         campaign's own files, so an absent checkout is a failure and never a skip.",
        root.display()
    );
    root
}

/// Every declared zone of every campaign: `(label, program, region, seed)`.
fn declared_zones() -> Vec<(String, Program, [u32; 3], u64)> {
    let mut out = Vec::new();
    let mut campaigns: Vec<PathBuf> = std::fs::read_dir(content_campaigns())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    campaigns.sort();
    for campaign in campaigns {
        let programs = campaign.join("design").join("programs");
        let manifest = programs.join("zones.json");
        if !manifest.is_file() {
            continue;
        }
        let name = campaign.file_name().unwrap().to_string_lossy().to_string();
        let parsed: ZoneManifest =
            serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        for zone in parsed.zones {
            let path = programs.join(&zone.program);
            let program: Program = serde_json::from_slice(&std::fs::read(&path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            out.push((
                format!("{name}/{}", zone.id),
                program,
                zone.region,
                zone.seed,
            ));
        }
    }
    out
}

/// The corpus is not empty, and it is the size the campaign claims.
///
/// A binding count, stated rather than assumed: every assertion below iterates
/// this list, so a list that silently became empty would turn each of them into
/// a green that examined nothing.
#[test]
fn the_declared_zone_corpus_is_not_empty() {
    let zones = declared_zones();
    assert!(
        !zones.is_empty(),
        "no campaign declares a zone — every sweep in this file binds to nothing"
    );
    assert_eq!(
        zones.len(),
        8,
        "the drowned-bell remake declares eight zones: {:?}",
        zones.iter().map(|z| &z.0).collect::<Vec<_>>()
    );
}

/// Every declared zone is a structurally valid program: every `call` resolves,
/// every role and param it names exists. Found here rather than at expansion,
/// with no region and no seed involved.
#[test]
fn every_declared_zone_is_structurally_valid() {
    for (label, program, _, _) in declared_zones() {
        program
            .validate()
            .unwrap_or_else(|e| panic!("{label}: {e}"));
    }
}

/// **ADR-0006 over the campaign's own files: same program, same region, same
/// seed, byte-identical model.**
///
/// The second of the two methods this is proved by, and deliberately a different
/// code path from the first. The first runs `delve-grammar expand` twice as
/// separate processes and compares the CONTENT hash of every file each wrote —
/// the `.nbt` through the gzip writer, the metadata, the gate report. This one
/// stays in process and compares `VoxelModel::canonical_bytes`, so it touches no
/// NBT writer, no compressor and no filesystem. A nondeterminism that both miss
/// would have to survive two encodings.
///
/// The negative half is asserted too: a different seed must produce different
/// bytes for at least one zone. Without it, "identical" would also be satisfied
/// by an expander that ignored the program.
#[test]
fn every_declared_zone_expands_byte_identically_twice() {
    let mut seed_sensitive = 0usize;
    for (label, program, region, seed) in declared_zones() {
        let box3 = Box3::at_origin(region);
        let a = expand(&program, box3, &ExpandOptions::seeded(seed))
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        let b = expand(&program, box3, &ExpandOptions::seeded(seed))
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        assert_eq!(
            a.model.canonical_bytes(),
            b.model.canonical_bytes(),
            "{label} is not deterministic at seed {seed}"
        );
        assert_eq!(a.anchors, b.anchors, "{label}: anchors moved");

        let other = expand(&program, box3, &ExpandOptions::seeded(seed.wrapping_add(1)))
            .unwrap_or_else(|e| panic!("{label}: {e}"));
        if other.model.canonical_bytes() != a.model.canonical_bytes() {
            seed_sensitive += 1;
        }
    }
    assert!(
        seed_sensitive > 0,
        "no declared zone changed when its seed did, so the equality above would hold \
         for an expander that read neither the seed nor the program"
    );
}

/// The manifest and the directory agree in both directions.
///
/// The direction that rots is the second: a zone program dropped into
/// `design/programs/` with no manifest entry is a program nothing expands and
/// nothing checks, and it looks exactly like a program that is fine. The CLI
/// reds on it too (`delve-grammar audit`); this is the same claim inside
/// `cargo test`, because the two run in different jobs.
#[test]
fn every_program_file_is_declared_and_every_declaration_has_a_file() {
    let mut campaigns: Vec<PathBuf> = std::fs::read_dir(content_campaigns())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    campaigns.sort();
    let mut checked = 0usize;
    for campaign in campaigns {
        let programs = campaign.join("design").join("programs");
        if !programs.is_dir() {
            continue;
        }
        let mut on_disk: Vec<String> = std::fs::read_dir(&programs)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".json") && n != "zones.json")
            .collect();
        on_disk.sort();
        let manifest = programs.join("zones.json");
        assert!(
            manifest.is_file(),
            "{} holds {} zone program(s) and no zones.json, so nothing states the region \
             and seed they are built at",
            programs.display(),
            on_disk.len()
        );
        let parsed: ZoneManifest =
            serde_json::from_slice(&std::fs::read(&manifest).unwrap()).unwrap();
        let mut declared: Vec<String> = parsed.zones.iter().map(|z| z.program.clone()).collect();
        declared.sort();
        assert_eq!(on_disk, declared, "{}", programs.display());
        checked += on_disk.len();
    }
    assert!(checked > 0, "no zone program was examined");
}

/// A zone's id is unique inside its campaign — the audit reports by that id, and
/// two zones sharing one would make an exclusion record ambiguous.
#[test]
fn every_declared_zone_id_is_unique() {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for (label, _, _, _) in declared_zones() {
        *seen.entry(label).or_default() += 1;
    }
    let dupes: Vec<&String> = seen
        .iter()
        .filter(|(_, n)| **n > 1)
        .map(|(k, _)| k)
        .collect();
    assert!(dupes.is_empty(), "duplicate zone ids: {dupes:?}");
}
