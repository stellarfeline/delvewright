//! **The admission checks, re-run over the library that is already admitted.**
//!
//! `delve-admit audit` binds to the moment a piece *enters* the library, which
//! is exactly one moment per piece, forever. Everything that changes afterwards
//! — a new check, a new pinned version, a rule nobody had written yet — reaches
//! the pieces admitted after it and no others. That is the fourth vacuity mode
//! CLAUDE.md names: the gate is correct, it is invoked, and it protects nothing
//! it did not already protect.
//!
//! It is not hypothetical here. The shape rule (`DW0735`) landed after most of
//! the library was admitted, so it had never been asked of those pieces; this
//! sweep asked, and it named seven pieces whose fences, walls, iron bars, vines
//! and glow lichen omitted a property carrying the block's shape — a portcullis
//! that was fifteen isolated posts, a meadow boundary that was ten separate
//! stubs. Those pieces are re-exported at the pinned content SHA, so what the
//! sweep protects now is that no eighth arrives.
//!
//! The allowlist had the same gap and a worse consequence: nothing ran it over
//! the library either, so the tool disagreeing with ITSELF about one block went
//! unmeasured. `DW0734` passed `hero-temple-ruin-arch.nbt`'s `minecraft:chain`
//! as a datafixable warning while `DW0730` refused the identical cell as
//! not-allowlisted, because the allowlist is a list of names at the pin and was
//! being asked about a name written in a 1.18.2 vocabulary. Judging the id the
//! game will actually load is the fix, and this file is where it is held.
//!
//! So these are the sweeps, bound to `cargo test` (the required `rust` job)
//! rather than to a doc line. Each states its binding count: a run that examined
//! zero prefabs is a failure, not a pass. Warnings are printed rather than
//! swallowed, so the one pre-pin id in the library stays visible without being
//! a refusal, and the id it resolves to is printed beside it.

use std::collections::BTreeMap;
use std::path::PathBuf;

use delvewright_admit::allowlist::Allowlist;
use delvewright_admit::spatial::{self, Door};
use delvewright_admit::structure::Structure;
use delvewright_schem::blocks::{BlockRegistry, StateJudgement};
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

/// **Every block state in the shipped library is judged against the pin, at the
/// DataVersion its own file declares.**
///
/// The verdict is `BlockRegistry::judge_at`, the same rule `delve-admit audit`
/// applies at admission: a state the pin does not have is an error when the file
/// claims the pin or later (`DW0733`, no datafix will run and the block loads as
/// air), and a warning when the file pre-dates it (`DW0734`, load-time
/// datafixing is expected to migrate it). The warnings are counted and printed
/// rather than asserted away, so a pre-pin id no fixer maps is still visible
/// here instead of being silently absorbed.
#[test]
fn every_shipped_prefab_places_blocks_the_pinned_version_has() {
    let registry = BlockRegistry::v1_21_11();
    let library = library();
    let mut states = 0usize;
    let mut bad: Vec<String> = Vec::new();
    let mut warned: Vec<String> = Vec::new();

    for (file, s) in &library {
        for entry in &s.palette {
            states += 1;
            match registry.judge_at(&entry.name, &entry.properties, s.data_version) {
                StateJudgement::Valid => {}
                StateJudgement::InvalidAtPin(e) => bad.push(format!("{file}: {e}")),
                StateJudgement::PrePin(e) => {
                    warned.push(format!("{file} (DataVersion {}): {e}", s.data_version))
                }
            }
        }
    }

    assert!(
        !library.is_empty() && states > 0,
        "binding count is zero: {} prefab(s), {states} palette entr(ies) examined",
        library.len()
    );
    println!(
        "DW0733 sweep: {} prefab(s), {states} palette entr(ies) examined; {} pre-pin warning(s) \
         (DW0734)",
        library.len(),
        warned.len()
    );
    for w in &warned {
        println!("  DW0734 {w}");
    }
    assert!(
        bad.is_empty(),
        "{} of {} palette entr(ies), over {} shipped prefab(s), name a block state Minecraft \
         1.21.11 does not have in a file that claims the pin:\n  {}",
        bad.len(),
        states,
        library.len(),
        bad.join("\n  ")
    );
}

/// **Every block state in the library writes the properties that carry its shape.**
///
/// The `DW0735` rule, re-run over what is already admitted: a property named by
/// the block's own `multipart` selectors *assembles* the model, so a state that
/// omits one places disconnected — a wall or a fence as an isolated post — and
/// the file says nothing about it. A `variants` property is not in scope; its
/// default picks a complete model, which is what the author meant.
///
/// Scoped to templates whose `DataVersion` is the pin, for the same reason the
/// id verdict is: an older palette is DataFixerUpper's business. The binding
/// count is asserted separately for the pinned subset — a library that became
/// all-legacy would otherwise pass by examining nothing.
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
            // Spelling is the other test's verdict; this one is shape.
            let omitted = registry.omitted_shape_carrying(&entry.name, &entry.properties);
            if !omitted.is_empty() {
                bad.push(format!(
                    "{file}: {} omits {} ({used} cell(s))",
                    entry.name,
                    omitted.join(", ")
                ));
            }
        }
    }

    assert!(
        at_pin > 0 && states > 0,
        "binding count is zero: {at_pin} prefab(s) at DataVersion {PINNED_DATA_VERSION}, \
         {states} used palette entr(ies)"
    );
    println!(
        "DW0735 sweep: {at_pin} prefab(s) at the pin, {states} used palette entr(ies) examined"
    );
    assert!(
        bad.is_empty(),
        "{} of {} used palette entr(ies), over {} of {} shipped prefab(s) written at the pinned \
         version, omit a property that carries the block's shape, so they place disconnected and \
         the file does not say so. The fix is to write the connection the piece actually wants — \
         not the default, which is the isolated post that made this a finding.\n  {}",
        bad.len(),
        states,
        at_pin,
        library.len(),
        bad.join("\n  ")
    );
}

/// **Every palette entry in the shipped library is judged by the allowlist over
/// the id the game will actually load.**
///
/// The allowlist is a list of names at the pin, and four of the library's
/// pieces are pre-pin third-party assets whose palettes are written in an older
/// vocabulary the game renames on load. Judged as written, one of them was
/// refused by the allowlist in the same run in which the spelling rule passed
/// it as a datafixable warning — one tool, two verdicts, on one block. That
/// contradiction had never been measured over the library because no test ran
/// the allowlist over the library at all: `delve-admit audit` binds at the
/// moment a piece enters it, which is one moment per piece, forever.
///
/// So this is the allowlist's own sweep, bound to `cargo test`. It states its
/// binding count, and it is deliberately the DEFAULT allowlist: a bespoke list
/// written to fit the corpus would be the check answering itself.
#[test]
fn every_shipped_prefab_is_allowlisted_as_the_game_will_load_it() {
    let registry = BlockRegistry::v1_21_11();
    let allow = Allowlist::default_building();
    let library = library();
    let mut entries = 0usize;
    let mut renamed = 0usize;
    let mut bad: Vec<String> = Vec::new();

    for (file, s) in &library {
        for entry in &s.palette {
            entries += 1;
            let judged = allow.judge_entry(entry, registry, s.data_version);
            if let Some(written) = judged.renamed_from {
                renamed += 1;
                println!(
                    "  {file} (DataVersion {}): {written} loads as {}",
                    s.data_version, judged.judged
                );
            }
            if !judged.permitted {
                bad.push(format!(
                    "{file}: {} (written {})",
                    judged.judged, entry.name
                ));
            }
        }
    }

    assert!(
        !library.is_empty() && entries > 0,
        "binding count is zero: {} prefab(s), {entries} palette entr(ies) examined",
        library.len()
    );
    println!(
        "DW0730 sweep: {} prefab(s), {entries} palette entr(ies) examined; {renamed} resolved \
         through a datafix rename",
        library.len()
    );
    assert!(
        bad.is_empty(),
        "{} of {entries} palette entr(ies), over {} shipped prefab(s), name a block the palette \
         allowlist does not permit — judged as the pinned game will hold it, not as the bytes \
         spell it:\n  {}",
        bad.len(),
        library.len(),
        bad.join("\n  ")
    );
}

/// **The spatial contract's second door, opened on every shipped prefab.**
///
/// `delve-admit audit` opens it at the moment a piece enters the library, which
/// is one moment per piece, forever — so a document that loses its contract
/// afterwards, to a hand edit or to a tool that models fewer fields than the
/// document has, is never asked again. This sweep asks, on every `cargo test`.
///
/// It states three numbers, and the third is the one to read: how many pieces
/// were examined, how many the door **refused**, and how many actually declare a
/// contract. That last is the door's own binding count, and it is currently
/// ZERO over the whole library — every shipped piece predates the contract, so
/// the second door is judging nothing here yet, and the compiler's `DW0781`
/// says the same thing about the same corpus from the placement side. Printed
/// rather than asserted, because requiring a contract of every prefab is a
/// version-adoption decision and not this gate's to make; asserted the moment a
/// piece has one, which is what the adoption round will turn this into.
#[test]
fn the_second_door_is_opened_on_every_shipped_prefab() {
    let dir = prefabs_dir();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|_| {
            panic!(
                "the prefab library is not at {} — the `campaigns/` content checkout is missing, \
                 so this sweep would examine nothing and pass",
                dir.display()
            )
        })
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("nbt"))
        .collect();
    paths.sort();

    let (mut judged, mut undeclared, mut no_document) = (0usize, 0usize, 0usize);
    let mut refused: Vec<String> = Vec::new();
    let mut failed: Vec<String> = Vec::new();
    let mut objects = 0usize;

    for path in &paths {
        let s = Structure::read(&std::fs::read(path).expect("the library file is readable"))
            .unwrap_or_else(|e| panic!("{} does not parse: {e}", path.display()));
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let door = Door::open(&spatial::grid(&s), 1, &path.with_extension("json"));
        objects += door.binding().objects;
        match &door {
            Door::Judged { report, .. } => {
                judged += 1;
                for gate in report.gates.iter().filter(|g| !g.passed()) {
                    failed.push(format!("{name}: {} — {}", gate.id, gate.detail));
                }
            }
            Door::Undeclared { .. } => undeclared += 1,
            Door::NoDocument { .. } => no_document += 1,
            Door::Refused { reason, .. } => refused.push(format!("{name}: {reason}")),
        }
    }

    assert!(
        !paths.is_empty(),
        "binding count is zero: no prefab was examined at all"
    );
    println!(
        "second-door sweep: {} prefab(s) examined; {judged} declare a contract and were judged \
         ({objects} object(s) examined by their obligations), {undeclared} declare none, \
         {no_document} carry no document, {} refused",
        paths.len(),
        refused.len()
    );
    if judged == 0 {
        println!(
            "  FINDING: the second door's binding over this library is ZERO — no shipped prefab \
             declares a spatial contract yet, so nothing here is being judged by it. The \
             adoption round that gives the library contracts is what turns this into a number."
        );
    }
    assert!(
        refused.is_empty(),
        "{} shipped prefab(s) cannot be judged by the second door at all — a declaration \
         document that does not parse, or one that declares no contract while its own anchors \
         resolve into contract elements (which means the declaration was dropped):\n  {}",
        refused.len(),
        refused.join("\n  ")
    );
    assert!(
        failed.is_empty(),
        "{} shipped prefab(s) declare a spatial contract their own blocks do not honour:\n  {}",
        failed.len(),
        failed.join("\n  ")
    );
}
