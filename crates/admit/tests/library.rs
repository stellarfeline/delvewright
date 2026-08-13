//! **The admission checks, re-run over the library that is already admitted.**
//!
//! `delve-admit audit` binds to the moment a piece *enters* the library, which
//! is exactly one moment per piece, forever. Everything that changes afterwards
//! — a new check, a new pinned version, a rule nobody had written yet — reaches
//! the pieces admitted after it and no others. That is the fourth vacuity mode
//! CLAUDE.md names: the gate is correct, it is invoked, and it protects nothing
//! it did not already protect.
//!
//! It is not hypothetical here. `crates/schem/src/blocks.rs` has recorded since
//! it was written that `hero-temple-ruin-arch.nbt` carries `minecraft:chain`, an
//! id 1.21.11 does not have — the note names the file and the cell. The audit
//! that would refuse it today has never been pointed at it, because the piece
//! was admitted before the check existed and nothing re-asks.
//!
//! So this is the sweep, bound to `cargo test` (the required `rust` job) rather
//! than to a doc line. It states its binding count: a run that examined zero
//! prefabs is a failure, not a pass.

use std::collections::BTreeMap;
use std::path::PathBuf;

use delvewright_admit::structure::Structure;
use delvewright_schem::blocks::{BlockError, BlockRegistry};
use delvewright_schem::convert::DATA_VERSION as PINNED_DATA_VERSION;

/// The shipped prefab library. Lives in the content repo
/// (`delvewright-campaigns`), reached at `campaigns/prefabs` — the `campaigns/`
/// symlink locally, a content-repo checkout at the pinned SHA in CI.
fn prefabs_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../campaigns/prefabs")
}

/// Every `.nbt` in the library, as `(file name, structure)`.
fn library() -> Vec<(String, Structure)> {
    let dir = prefabs_dir();
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        panic!(
            "the prefab library is not at {} — the `campaigns/` content checkout is missing, so \
             this sweep would examine nothing and pass",
            dir.display()
        );
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("nbt"))
        .collect();
    paths.sort();
    for path in paths {
        let bytes = std::fs::read(&path).expect("the library file is readable");
        let s = Structure::read(&bytes)
            .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
        out.push((path.file_name().unwrap().to_string_lossy().into_owned(), s));
    }
    out
}

/// **Every block state in the shipped library is a state Minecraft 1.21.11 has.**
///
/// Spelling only, at every DataVersion — an unknown id is worth reporting
/// whatever version wrote it, because nothing in this repo runs vanilla's
/// DataFixerUpper and every tool that reads the file sees the id as written.
#[test]
fn every_shipped_prefab_places_blocks_the_pinned_version_has() {
    let registry = BlockRegistry::v1_21_11();
    let library = library();
    let mut states = 0usize;
    let mut bad: Vec<String> = Vec::new();

    for (file, s) in &library {
        for entry in &s.palette {
            states += 1;
            if let Err(e) = registry.validate(&entry.name, &entry.properties) {
                bad.push(format!("{file}: {e}"));
            }
        }
    }

    assert!(
        !library.is_empty() && states > 0,
        "binding count is zero: {} prefab(s), {states} palette entr(ies) examined",
        library.len()
    );
    assert!(
        bad.is_empty(),
        "{} of {} palette entr(ies), over {} shipped prefab(s), name a block state Minecraft \
         1.21.11 does not have:\n  {}",
        bad.len(),
        states,
        library.len(),
        bad.join("\n  ")
    );
}

/// **Every block state written at the pinned version says what it is.**
///
/// Scoped to templates whose `DataVersion` is the pin: an older palette is
/// upgraded by vanilla's DataFixerUpper on load and the pinned registry has no
/// authority over it (the three third-party pieces are all pre-pin). The
/// binding count is asserted for the same reason as above, and separately for
/// the pinned subset — a library that became all-legacy would otherwise make
/// this pass by examining nothing.
#[test]
fn every_shipped_prefab_written_at_the_pin_states_every_property() {
    let registry = BlockRegistry::v1_21_11();
    let library = library();
    let mut at_pin = 0usize;
    let mut states = 0usize;
    let mut bad: Vec<String> = Vec::new();

    for (file, s) in &library {
        if s.data_version != PINNED_DATA_VERSION {
            continue;
        }
        at_pin += 1;
        // Cells per palette entry, so the report says how much of the piece is
        // affected rather than how many distinct entries are.
        let mut cells: BTreeMap<usize, usize> = BTreeMap::new();
        for b in &s.blocks {
            *cells.entry(b.state as usize).or_insert(0) += 1;
        }
        for (i, entry) in s.palette.iter().enumerate() {
            let used = cells.get(&i).copied().unwrap_or(0);
            if used == 0 {
                continue;
            }
            states += 1;
            // Spelling is the other test's verdict; this one is completeness.
            if let Err(e @ BlockError::UnderSpecified { .. }) =
                registry.validate_complete(&entry.name, &entry.properties)
            {
                bad.push(format!("{file}: {e} ({used} cell(s))"));
            }
        }
    }

    assert!(
        at_pin > 0 && states > 0,
        "binding count is zero: {at_pin} prefab(s) at DataVersion {PINNED_DATA_VERSION}, \
         {states} used palette entr(ies)"
    );
    assert!(
        bad.is_empty(),
        "{} of {} used palette entr(ies), over {} of {} shipped prefab(s) written at the pinned \
         version, do not state every property of the block they place. Vanilla fills those from \
         the block's default state; the renderer, the prefab viewer, the occupancy pass and \
         `delve-admit` cannot. Regenerate the affected tilesets — the generators now write the \
         complete state (`invariants::complete_state`), and the completion is lossless, so the \
         world is unchanged.\n  {}",
        bad.len(),
        states,
        at_pin,
        library.len(),
        bad.join("\n  ")
    );
}
